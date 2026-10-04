# Files

- [Bootstrap, SystemContext and Shared Services](bootstrap-and-services.md) - The single canonical initialization sequence that builds SystemContext, DownloadManager, the lazy BT backend, the BackendRegistry, CDN service and Dispatcher, plus the shared services that own global runtime state and the two frontends that consume them.
- [Workspace and System Architecture](overview.md) - Repository layout and runtime topology of limedl — the core engine, Slint desktop and headless server crates, protocol routing by TaskId, the typed EventBus fan-out, and the cross-cutting serialization and build conventions.
- [Protocol Routing and the Dispatcher Facade](protocol-routing-and-dispatcher.md) - The protocol abstraction layer of limedl — the DownloadBackend trait, BackendRegistry routing by TaskId, and the Dispatcher facade that unifies lifecycle, settings, disk, concurrency and protocol-specific operations while auto-emitting state events.
