import * as http from "node:http";
import * as path from "node:path";
import * as fs from "node:fs";
import * as os from "node:os";

const CHUNK_SIZE = 64 * 1024; // 64 KB write chunks

/**
 * HTTP server for controlled download testing.
 *
 * Serves deterministic test files with HTTP Range support
 * and configurable per-connection speed limiting.
 */
export class TestFileServer {
  private server: http.Server | null = null;
  private readonly baseDir: string;
  private _port = 0;

  /** Map of filename → absolute path */
  readonly files: Record<string, string> = {};

  constructor(
    baseDir?: string,
    private readonly speedLimitBps = 0, // 0 = unlimited
  ) {
    this.baseDir = baseDir ?? fs.mkdtempSync(path.join(os.tmpdir(), "limedl-e2e-"));
  }

  get port(): number {
    return this._port;
  }

  get url(): string {
    return `http://localhost:${this._port}`;
  }

  /**
   * Returns the full download URL for a given filename.
   * Throws if the filename is not a known test file.
   */
  getUrl(filename: string): string {
    if (!this.files[filename]) {
      throw new Error(
        `Unknown test file: "${filename}". Known files: ${Object.keys(this.files).join(", ")}`,
      );
    }
    return `${this.url}/${filename}`;
  }

  /**
   * Create test files and start listening.
   */
  async start(port = 9876): Promise<void> {
    this.generateFiles();

    return new Promise((resolve, reject) => {
      this.server = http.createServer((req, res) => {
        this.handleRequest(req, res);
      });

      const onError = (err: Error) => reject(err);
      this.server.once("error", onError);

      this.server.listen(port, () => {
        this.server!.removeListener("error", onError);
        // Add a logging handler for subsequent errors
        this.server!.on("error", (err) => console.error("[TestFileServer] runtime error:", err));

        this._port = (this.server!.address() as import("node:net").AddressInfo).port;
        // Store on globalThis for teardown access
        (globalThis as any).__TEST_FILE_SERVER__ = this;
        resolve();
      });
    });
  }

  /**
   * Stop the server and clean up.
   */
  stop(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.server) {
        resolve();
        return;
      }
      this.server.close((err) => {
        if (err) reject(err);
        else resolve();
      });
    });
  }

  /**
   * Remove the temporary directory and all test files.
   */
  cleanup(): void {
    try {
      fs.rmSync(this.baseDir, { recursive: true, force: true });
    } catch {
      // best-effort cleanup
    }
  }

  // ----------------------------------------------------------------
  // Private helpers
  // ----------------------------------------------------------------

  private generateFiles(): void {
    fs.mkdirSync(this.baseDir, { recursive: true });

    // small.txt — small text file
    const smallPath = path.join(this.baseDir, "small.txt");
    const smallContent = "Hello from limedl test file server!\n";
    fs.writeFileSync(smallPath, smallContent, "utf-8");
    this.files["small.txt"] = smallPath;

    // 1mb.bin — 1 MiB of deterministic content (repeating "FLARE" pattern)
    const oneMbPath = path.join(this.baseDir, "1mb.bin");
    this.writeDeterministicFile(oneMbPath, 1024 * 1024, "LIMEDL1");
    this.files["1mb.bin"] = oneMbPath;

    // 10mb.bin — 10 MiB
    const tenMbPath = path.join(this.baseDir, "10mb.bin");
    this.writeDeterministicFile(tenMbPath, 10 * 1024 * 1024, "LIMEDL10");
    this.files["10mb.bin"] = tenMbPath;

    // 50mb.bin — 50 MiB (for segmented download tests)
    const fiftyMbPath = path.join(this.baseDir, "50mb.bin");
    this.writeDeterministicFile(fiftyMbPath, 50 * 1024 * 1024, "LIMEDL50");
    this.files["50mb.bin"] = fiftyMbPath;
  }

  private writeDeterministicFile(filePath: string, size: number, pattern: string): void {
    const fd = fs.openSync(filePath, "w");
    const patternBuf = Buffer.from(pattern, "utf-8");
    const fullChunks = Math.floor(size / patternBuf.length);
    const remainder = size % patternBuf.length;

    // Write full pattern chunks (64KB at a time to avoid huge allocations)
    const maxWrite = 64 * 1024;
    let written = 0;
    while (written < fullChunks * patternBuf.length) {
      const toWrite = Math.min(maxWrite, fullChunks * patternBuf.length - written);
      const buf = Buffer.allocUnsafe(toWrite);
      for (let i = 0; i < toWrite; i += patternBuf.length) {
        patternBuf.copy(buf, i, 0, Math.min(patternBuf.length, toWrite - i));
      }
      fs.writeSync(fd, buf, 0, toWrite);
      written += toWrite;
    }

    // Write remainder
    if (remainder > 0) {
      fs.writeSync(fd, patternBuf.subarray(0, remainder));
    }

    fs.closeSync(fd);
  }

  /** Prevent directory traversal — ensures the path is inside baseDir. */
  private safePath(requestedPath: string): string | null {
    // Normalize and resolve
    const resolved = path.resolve(this.baseDir, requestedPath);

    // Must be within baseDir
    const baseWithSep = this.baseDir.endsWith(path.sep) ? this.baseDir : this.baseDir + path.sep;
    if (!resolved.startsWith(baseWithSep) && resolved !== this.baseDir) {
      return null;
    }

    // Must exist and be a file (not a directory)
    try {
      const stat = fs.statSync(resolved);
      if (!stat.isFile()) return null;
    } catch {
      return null;
    }

    return resolved;
  }

  private handleRequest(req: http.IncomingMessage, res: http.ServerResponse): void {
    // Only GET requests
    if (req.method !== "GET") {
      res.writeHead(405, { Allow: "GET" });
      res.end();
      return;
    }

    // Parse the URL path, strip leading slash
    const urlPath = decodeURIComponent(req.url ?? "/").replace(/^\//, "");
    if (!urlPath || urlPath.includes("/")) {
      res.writeHead(404);
      res.end("Not found");
      return;
    }

    const filePath = this.safePath(urlPath);
    if (!filePath) {
      res.writeHead(404);
      res.end("Not found");
      return;
    }

    const stat = fs.statSync(filePath);
    const fileSize = stat.size;
    const fileName = path.basename(filePath);

    // Parse Range header
    const rangeHeader = req.headers.range;
    let start = 0;
    let end = fileSize - 1;
    let statusCode = 200;

    if (rangeHeader) {
      const parsed = this.parseRange(rangeHeader, fileSize);
      if (parsed) {
        start = parsed.start;
        end = parsed.end;
        statusCode = 206;
      } else {
        // Invalid range — return 416
        res.writeHead(416, {
          "Content-Range": `bytes */${fileSize}`,
        });
        res.end();
        return;
      }
    }

    const contentLength = end - start + 1;
    const headers: Record<string, string> = {
      "Content-Type": "application/octet-stream",
      "Content-Disposition": `attachment; filename="${fileName}"`,
      "Accept-Ranges": "bytes",
      "Content-Length": String(contentLength),
    };

    if (statusCode === 206) {
      headers["Content-Range"] = `bytes ${start}-${end}/${fileSize}`;
    }

    res.writeHead(statusCode, headers);

    // Stream the file with optional speed limiting
    this.streamFile(res, filePath, start, end).catch((err) => {
      console.error("[TestFileServer] streamFile error:", err);
      if (!res.headersSent) {
        res.writeHead(500);
        res.end("Internal server error");
      }
    });
  }

  private parseRange(rangeHeader: string, fileSize: number): { start: number; end: number } | null {
    const match = /^bytes=(\d+)-(\d*)$/.exec(rangeHeader);
    if (!match) return null;

    const start = Number.parseInt(match[1], 10);
    let end = match[2] ? Number.parseInt(match[2], 10) : fileSize - 1;

    if (Number.isNaN(start) || start >= fileSize) return null;
    if (Number.isNaN(end) || end >= fileSize) end = fileSize - 1;
    if (start > end) return null;

    return { start, end };
  }

  private async streamFile(
    res: http.ServerResponse,
    filePath: string,
    start: number,
    end: number,
  ): Promise<void> {
    const fd = fs.openSync(filePath, "r");
    let position = start;
    const limiter = this.speedLimitBps > 0 ? new SpeedLimiter(this.speedLimitBps) : null;

    try {
      while (position <= end) {
        const remaining = end - position + 1;
        const chunkSize = Math.min(CHUNK_SIZE, remaining);
        const buf = Buffer.allocUnsafe(chunkSize);
        const bytesRead = fs.readSync(fd, buf, 0, chunkSize, position);

        if (bytesRead <= 0) break;

        const writeBuf = bytesRead < chunkSize ? buf.subarray(0, bytesRead) : buf;

        if (limiter) {
          await limiter.write(res, writeBuf);
        } else {
          await new Promise<void>((resolve, reject) => {
            res.write(writeBuf, (err) => {
              if (err) reject(err);
              else resolve();
            });
          });
        }

        position += bytesRead;
      }
    } finally {
      fs.closeSync(fd);
    }

    res.end();
  }
}

/**
 * Simple token-bucket speed limiter.
 * Tracks bytes written vs. elapsed time and inserts delays
 * when actual throughput exceeds the target.
 */
class SpeedLimiter {
  private bytesWritten = 0;
  private lastCheck = Date.now();

  constructor(private readonly bps: number) {}

  async write(res: http.ServerResponse, chunk: Buffer): Promise<void> {
    const now = Date.now();
    const elapsed = (now - this.lastCheck) / 1000;
    const allowedBytes = elapsed * this.bps;

    this.bytesWritten += chunk.length;

    if (this.bytesWritten > allowedBytes && this.bps > 0) {
      const excessBytes = this.bytesWritten - allowedBytes;
      const waitMs = Math.min((excessBytes / this.bps) * 1000, 1000);
      if (waitMs > 1) {
        await new Promise((r) => setTimeout(r, waitMs));
      }
      // Reset accounting after delay
      this.lastCheck = Date.now();
      this.bytesWritten = 0;
    }

    return new Promise<void>((resolve, reject) => {
      res.write(chunk, (err) => {
        if (err) reject(err);
        else resolve();
      });
    });
  }
}
