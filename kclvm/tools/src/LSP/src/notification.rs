use lsp_types::notification::{
    DidChangeTextDocument,
    DidChangeWatchedFiles,
    DidCloseTextDocument,
    DidOpenTextDocument,
    DidSaveTextDocument, //DidDeleteFiles, DidRenameFiles, DidCreateFiles, //todo more
};
use std::path::Path;

use crate::{
    dispatcher::NotificationDispatcher,
    from_lsp,
    state::LanguageServerState,
    util::apply_document_changes,
    util::{build_word_index_for_file_content, word_index_add, word_index_subtract},
};

impl LanguageServerState {
    pub fn on_notification(
        &mut self,
        notification: lsp_server::Notification,
    ) -> anyhow::Result<()> {
        self.log_message(format!("on notification {:?}", notification));
        NotificationDispatcher::new(self, notification)
            .on::<DidOpenTextDocument>(LanguageServerState::on_did_open_text_document)?
            .on::<DidChangeTextDocument>(LanguageServerState::on_did_change_text_document)?
            .on::<DidSaveTextDocument>(LanguageServerState::on_did_save_text_document)?
            .on::<DidCloseTextDocument>(LanguageServerState::on_did_close_text_document)?
            .on::<DidChangeWatchedFiles>(LanguageServerState::on_did_change_watched_files)?
            .finish();
        Ok(())
    }

    /// Called when a `DidOpenTextDocument` notification was received.
    fn on_did_open_text_document(
        &mut self,
        params: lsp_types::DidOpenTextDocumentParams,
    ) -> anyhow::Result<()> {
        let path = from_lsp::abs_path(&params.text_document.uri)?;
        self.log_message(format!("on did open file: {:?}", path));

        self.vfs.write().set_file_contents(
            path.clone().into(),
            Some(params.text_document.text.into_bytes()),
        );

        if let Some(id) = self.vfs.read().file_id(&path.into()) {
            self.opened_files.insert(id);
        }
        Ok(())
    }

    /// Called when a `DidChangeTextDocument` notification was received.
    fn on_did_save_text_document(
        &mut self,
        params: lsp_types::DidSaveTextDocumentParams,
    ) -> anyhow::Result<()> {
        let lsp_types::DidSaveTextDocumentParams {
            text_document,
            text: _,
        } = params;

        let path = from_lsp::abs_path(&text_document.uri)?;
        self.log_message(format!("on did save file: {:?}", path));
        Ok(())
    }

    /// Called when a `DidChangeTextDocument` notification was received.
    fn on_did_change_text_document(
        &mut self,
        params: lsp_types::DidChangeTextDocumentParams,
    ) -> anyhow::Result<()> {
        let lsp_types::DidChangeTextDocumentParams {
            text_document,
            content_changes,
        } = params;

        let path = from_lsp::abs_path(&text_document.uri)?;
        self.log_message(format!("on did_change file: {:?}", path));

        // update vfs
        let vfs = &mut *self.vfs.write();
        let file_id = vfs
            .file_id(&path.clone().into())
            .ok_or(anyhow::anyhow!("Already checked that the file_id exists!"))?;

        let mut text = String::from_utf8(vfs.file_contents(file_id).to_vec())?;
        let old_text = text.clone();
        apply_document_changes(&mut text, content_changes);
        vfs.set_file_contents(path.into(), Some(text.clone().into_bytes()));

        // update word index
        let old_word_index = build_word_index_for_file_content(old_text, &text_document.uri);
        let new_word_index = build_word_index_for_file_content(text.clone(), &text_document.uri);
        let binding = text_document.uri.path();
        let file_path = Path::new(binding); //todo rename
        for (key, value) in &mut self.word_index_map {
            let workspace_folder_path = Path::new(key.path());
            if file_path.starts_with(workspace_folder_path) {
                word_index_subtract(value, old_word_index.clone());
                word_index_add(value, new_word_index.clone());
            }
        }
        // let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // let url = lsp_types::Url::from_file_path(root.clone()).unwrap();
        // let mm = self.word_index_map.get(&url).unwrap();
        // println!("word_index_map: {:?}", mm);

        // let file = from_lsp::file_path_from_url(&text_document.uri)?;
        // let old_word_index = build_word_index_for_file_content(old_text, &text_document.uri);
        // let new_word_index = build_word_index_for_file_content(text.clone(), &text_document.uri);

        // let file_path = Path::new(&text_document.uri.path());
        // for (key, mut value) in &self.word_index_map {
        //     let workspace_folder_path = Path::new(key.path());
        //     if file_path.starts_with(workspace_folder_path) {
        //         value = &word_index_subtract(value, old_word_index.clone());
        //         value = &word_index_add(value, new_word_index.clone());
        //     }
        // }

        Ok(())
    }

    /// Called when a `DidCloseTextDocument` notification was received.
    fn on_did_close_text_document(
        &mut self,
        params: lsp_types::DidCloseTextDocumentParams,
    ) -> anyhow::Result<()> {
        let path = from_lsp::abs_path(&params.text_document.uri)?;

        if let Some(id) = self.vfs.read().file_id(&path.clone().into()) {
            self.opened_files.remove(&id);
        }
        self.vfs_handle.invalidate(path);

        Ok(())
    }

    /// Called when a `DidChangeWatchedFiles` was received
    fn on_did_change_watched_files(
        &mut self,
        params: lsp_types::DidChangeWatchedFilesParams,
    ) -> anyhow::Result<()> {
        for change in params.changes {
            let path = from_lsp::abs_path(&change.uri)?;
            self.vfs_handle.invalidate(path);
        }
        Ok(())
    }
}
