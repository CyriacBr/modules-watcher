
pub struct SetupOptions {
    project: String,
    globEntries: Option<Vec<String>>,
    entries: Option<Vec<String>>,
    cacheDir: Option<String>
}

pub struct Watcher {
    setupOptions: SetupOptions
}

impl Watcher {
    pub fn setup(&self, opts: SetupOptions) -> Self {
        Watcher{setupOptions: opts}
    }
}