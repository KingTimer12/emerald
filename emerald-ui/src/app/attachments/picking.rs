//! Adding files: the picker, drops, pastes, and reading them within the limits.

use super::*;

impl Emerald {
    /// The + button: the system file picker.
    pub(in super::super) fn pick_files(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            this.update(cx, |this, cx| this.attach_paths(paths, cx)).ok();
        })
        .detach();
    }

    /// Files dropped on the chat.
    pub(in super::super) fn on_drop_files(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        self.attach_paths(paths.paths().to_vec(), cx);
    }

    /// Reads `paths` off the UI thread, refusing what is over the limit
    /// before reading it.
    pub(super) fn attach_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if !self.may_attach() {
            return;
        }
        let limit = self.upload_limit();
        let read = cx.background_spawn(async move {
            paths
                .into_iter()
                .filter(|path| path.is_file())
                .map(|path| read_file(path, limit))
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let files = read.await;
            this.update(cx, |this, cx| {
                for file in files {
                    match file {
                        Ok(upload) => this.attach(upload),
                        Err(Refused::TooLarge(name)) => {
                            let max = format_size(this.upload_limit());
                            this.attach_error(t!("upload.too_large", name = name, max = max));
                        }
                        Err(Refused::Unreadable(name)) => {
                            this.attach_error(t!("upload.unreadable", name = name));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// An image pasted into the message box.
    pub(in super::super) fn attach_pasted(&mut self, image: &Image, cx: &mut Context<Self>) {
        if !self.may_attach() {
            return;
        }
        if image.bytes.len() as u64 > self.upload_limit() {
            let max = format_size(self.upload_limit());
            self.attach_error(t!("upload.too_large", name = "image", max = max));
        } else {
            self.attach(Upload {
                name: format!("image.{}", extension(image.format)),
                bytes: image.bytes.clone().into(),
                spoiler: false,
                description: String::new(),
            });
        }
        cx.notify();
    }

    pub(super) fn attach(&mut self, upload: Upload) {
        if self.composer.attachments.len() >= MAX_FILES {
            self.attach_error(t!("upload.too_many", max = MAX_FILES));
            return;
        }
        self.composer.error = None;
        self.composer.attachments.push(Attachment::new(upload));
    }

    /// Why a file was left out, shown under the form it was meant for: the
    /// "New Post" form while it is open, else the message box.
    pub(super) fn attach_error(&mut self, error: impl Into<SharedString>) {
        match &mut self.overlay {
            Some(Overlay::CreatePost(modal)) => modal.error = Some(error.into()),
            _ => self.composer.error = Some(error.into()),
        }
    }

    /// The largest file the open channel takes, as the web client works it
    /// out: the better of the guild's boosts and the user's Nitro, and never
    /// under 20 MB. Discord checks again and refuses anything larger.
    pub(super) fn upload_limit(&self) -> u64 {
        let nitro = match self.people.me().and_then(|me| me.premium_type) {
            Some(2) => 500 * MB,
            Some(1 | 3) => 50 * MB,
            _ => 0,
        };
        let guild = self.spaces.current().info().and_then(|g| g.upload_limit()).unwrap_or(10 * MB);
        guild.max(nitro).max(20 * MB)
    }

    /// Whether files can go to the open channel at all.
    pub(super) fn may_attach(&self) -> bool {
        self.selected_channel.is_some_and(|channel| self.can_attach(channel))
    }
}

pub(super) fn read_file(path: PathBuf, limit: u64) -> Result<Upload, Refused> {
    let name = path
        .file_name()
        .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
    match std::fs::metadata(&path) {
        Ok(meta) if meta.len() > limit => return Err(Refused::TooLarge(name)),
        Ok(_) => {}
        Err(_) => return Err(Refused::Unreadable(name)),
    }
    match std::fs::read(&path) {
        // Re-checked: the file may have grown since.
        Ok(bytes) if bytes.len() as u64 > limit => Err(Refused::TooLarge(name)),
        Ok(bytes) => Ok(Upload {
            name,
            bytes: bytes.into(),
            spoiler: false,
            description: String::new(),
        }),
        Err(_) => Err(Refused::Unreadable(name)),
    }
}

/// The image format a card can preview, from the file name.
pub(super) fn preview_format(name: &str) -> Option<ImageFormat> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "gif" => Some(ImageFormat::Gif),
        "webp" => Some(ImageFormat::Webp),
        _ => None,
    }
}

pub(super) fn extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Gif => "gif",
        ImageFormat::Webp => "webp",
        ImageFormat::Svg => "svg",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        _ => "png",
    }
}
