use std::{
    fs::{self, File, OpenOptions},
    path::Path,
};

use fs4::fs_std::FileExt;

use super::error::{DownloadError, Result};

pub(super) fn open_download_file(path: &Path, total_size: Option<u64>) -> Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;

    preallocate_file(&file, total_size)?;
    Ok(file)
}

pub(super) fn reset_download_file(file: &File, total_size: Option<u64>) -> Result<()> {
    file.set_len(0)?;
    preallocate_file(file, total_size)
}

pub(super) fn finalize_temp_file(temp_path: &Path, destination_path: &Path) -> Result<()> {
    match fs::rename(temp_path, destination_path) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(temp_path, destination_path)?;
            fs::remove_file(temp_path)?;
            Ok(())
        }
    }
}

pub(super) fn write_all_at(file: &File, mut buffer: &[u8], mut offset: u64) -> Result<()> {
    while !buffer.is_empty() {
        let written = write_once_at(file, buffer, offset)?;
        if written == 0 {
            return Err(DownloadError::InvalidResponse(String::from(
                "failed to write download data",
            )));
        }
        offset += written as u64;
        buffer = &buffer[written..];
    }
    Ok(())
}

fn preallocate_file(file: &File, total_size: Option<u64>) -> Result<()> {
    let Some(total_size) = total_size else {
        return Ok(());
    };

    match file.allocate(total_size) {
        Ok(()) => Ok(()),
        Err(error) => match error.raw_os_error() {
            Some(38 | 45 | 95 | 524) => {
                file.set_len(total_size)?;
                Ok(())
            }
            _ => Err(error.into()),
        },
    }
}

#[cfg(unix)]
fn write_once_at(file: &File, buffer: &[u8], offset: u64) -> std::io::Result<usize> {
    use std::os::unix::fs::FileExt;

    file.write_at(buffer, offset)
}

#[cfg(windows)]
fn write_once_at(file: &File, buffer: &[u8], offset: u64) -> std::io::Result<usize> {
    use std::os::windows::fs::FileExt;

    file.seek_write(buffer, offset)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{open_download_file, reset_download_file, write_all_at};

    #[test]
    fn open_download_file_sizes_known_length() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("download.part");

        let file = open_download_file(&path, Some(1024 * 1024)).unwrap();

        assert_eq!(file.metadata().unwrap().len(), 1024 * 1024);
    }

    #[test]
    fn reset_download_file_restores_target_length() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("download.part");
        let file = open_download_file(&path, Some(4096)).unwrap();

        file.set_len(128).unwrap();
        reset_download_file(&file, Some(8192)).unwrap();

        assert_eq!(file.metadata().unwrap().len(), 8192);
    }

    #[test]
    fn write_after_preallocation_preserves_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("download.part");
        let file = open_download_file(&path, Some(4096)).unwrap();

        write_all_at(&file, b"test data", 0).unwrap();

        assert_eq!(file.metadata().unwrap().len(), 4096);
        let bytes = fs::read(path).unwrap();
        assert_eq!(&bytes[..9], b"test data");
    }
}
