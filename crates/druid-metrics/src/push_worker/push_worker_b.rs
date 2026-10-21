use super::*;

impl PushWorker {
    pub(crate) fn process_server_while_sending(
        &mut self,
        frame: ServerFrame,
    ) -> Result<(), PushWorkerError> {
        use crate::protocol::server_frame::Payload;

        if let Some(Payload::BatchAck(ack)) = frame.payload.as_ref() {
            if let Err(error) = self.acknowledge(ack.accepted_sequence) {
                tracing::warn!(sequence = ack.accepted_sequence, %error, "unexpected ACK");
            }
            return Ok(());
        }
        if self.deferred_server_frames.len() >= self.deferred_server_capacity {
            #[cfg(feature = "client")]
            if let (
                Some(progress),
                Some(crate::protocol::server_frame::Payload::Command(command)),
            ) = (&self.progress, frame.payload.as_ref())
            {
                let key = CommandWorkKey::from_command(command).map_err(ownership_error)?;
                progress
                    .drop_transport_command(&key)
                    .map_err(ownership_error)?;
            }
            return Err(PushWorkerError::ServerInboundOverload {
                capacity: self.deferred_server_capacity,
            });
        }
        self.deferred_server_frames.push_back(frame);
        Ok(())
    }

    pub(crate) fn begin_command_ownership(
        &self,
        command: &crate::protocol::Command,
    ) -> Result<Option<CommandWorkKey>, PushWorkerError> {
        self.progress
            .as_ref()
            .map(|progress| {
                progress
                    .begin_command(command)
                    .map(|(key, _transition)| key)
                    .map_err(ownership_error)
            })
            .transpose()
    }

    pub(crate) fn finish_command_ownership(
        &self,
        key: Option<&CommandWorkKey>,
    ) -> Result<(), PushWorkerError> {
        match (&self.progress, key) {
            (Some(progress), Some(key)) => progress.finish_command(key).map_err(ownership_error),
            (None, None) => Ok(()),
            _ => Err(PushWorkerError::Ownership(
                "command progress and work key must be present together".to_owned(),
            )),
        }
    }

    pub(crate) fn finish_command_after_reply(
        &self,
        sent: bool,
        key: Option<&CommandWorkKey>,
    ) -> bool {
        if !sent {
            return false;
        }
        if let Err(error) = self.finish_command_ownership(key) {
            tracing::error!(%error, "failed to finish command ownership");
            return false;
        }
        true
    }

    pub(crate) async fn process_server_frame(&mut self, frame: ServerFrame) -> bool {
        use crate::protocol::server_frame::Payload;

        match frame.payload {
            Some(Payload::BatchAck(ack)) => {
                if let Err(error) = self.acknowledge(ack.accepted_sequence) {
                    tracing::warn!(sequence = ack.accepted_sequence, %error, "unexpected ACK");
                }
                true
            }
            Some(Payload::Command(command))
                if self.window.is_full()
                    && matches!(
                        command.payload.as_ref(),
                        Some(crate::protocol::command::Payload::RequestFullSnapshot(_))
                    ) =>
            {
                if self.window.capacity() == 0 {
                    let key = match self.begin_command_ownership(&command) {
                        Ok(key) => key,
                        Err(error) => {
                            tracing::error!(%error, "failed to start command ownership");
                            return false;
                        }
                    };
                    let reply = self.make_command_ack(
                        command.command_id,
                        Err("ACK window capacity is zero".to_owned()),
                    );
                    let sent = self.send_outbound(reply).await.is_ok();
                    return self.finish_command_after_reply(sent, key.as_ref());
                }
                if self.deferred_server_frames.is_empty() {
                    self.deferred_server_frames.push_back(ServerFrame {
                        payload: Some(Payload::Command(command)),
                    });
                    true
                } else {
                    let key = match self.begin_command_ownership(&command) {
                        Ok(key) => key,
                        Err(error) => {
                            tracing::error!(%error, "failed to start command ownership");
                            return false;
                        }
                    };
                    let reply = self.make_command_ack(
                        command.command_id,
                        Err("another full snapshot command is waiting for ACK capacity".to_owned()),
                    );
                    let sent = self.send_outbound(reply).await.is_ok();
                    self.finish_command_after_reply(sent, key.as_ref())
                }
            }
            Some(Payload::Command(command))
                if matches!(
                    command.payload.as_ref(),
                    Some(crate::protocol::command::Payload::RequestFullSnapshot(_))
                ) =>
            {
                let key = match self.begin_command_ownership(&command) {
                    Ok(key) => key,
                    Err(error) => {
                        tracing::error!(%error, "failed to start command ownership");
                        return false;
                    }
                };
                self.handle_full_snapshot_command(command, key).await
            }
            Some(Payload::Command(command)) => {
                let key = match self.begin_command_ownership(&command) {
                    Ok(key) => key,
                    Err(error) => {
                        tracing::error!(%error, "failed to start command ownership");
                        return false;
                    }
                };
                let reply = self.handle_command(command);
                let sent = self.send_outbound(reply).await.is_ok();
                self.finish_command_after_reply(sent, key.as_ref())
            }
            Some(Payload::HelloAck(_)) | None => true,
            Some(Payload::ResyncRequired(resync)) => {
                tracing::warn!(
                    reason = resync.reason,
                    expected = resync.expected_sequence,
                    "server requested resync"
                );
                self.resync_required = true;
                false
            }
            Some(Payload::Error(error)) => {
                tracing::error!(code = error.code, message = error.message, "server error");
                !error.close_stream
            }
        }
    }

    pub(crate) fn handle_command(&mut self, command: crate::protocol::Command) -> ClientFrame {
        use crate::protocol::command::Payload;

        let command_id = command.command_id;
        let canonical = canonical_command_payload(&command.payload).map_err(str::to_owned);
        let result = canonical.and_then(|canonical| {
            match self.command_journal.begin(&command_id, canonical)? {
                (false, Some(result)) => result,
                (false, None) => Err("command is already executing".to_owned()),
                (true, None) => {
                    let result = match command.payload {
                        Some(Payload::ResetStats(reset)) => self
                            .command_executor
                            .as_ref()
                            .ok_or_else(|| "datasource registry is not configured".to_owned())
                            .and_then(|executor| {
                                executor
                                    .reset_stats(&reset.target_datasource_ids)
                                    .map_err(|error| error.to_string())
                            }),
                        Some(Payload::RequestFullSnapshot(_)) => {
                            Err("full snapshot command must use the durable path".to_owned())
                        }
                        None => Err("empty command payload".to_owned()),
                    };
                    self.command_journal.complete(&command_id, result.clone());
                    result
                }
                (true, Some(_)) => Err("new command unexpectedly had a result".to_owned()),
            }
        });
        self.make_command_ack(command_id, result)
    }

    pub(crate) async fn handle_full_snapshot_command(
        &mut self,
        command: crate::protocol::Command,
        ownership_key: Option<CommandWorkKey>,
    ) -> bool {
        let command_id = command.command_id;
        let canonical = match canonical_command_payload(&command.payload) {
            Ok(canonical) => canonical,
            Err(error) => {
                let reply = self.make_command_ack(command_id, Err(error.to_owned()));
                let sent = self.send_outbound(reply).await.is_ok();
                return self.finish_command_after_reply(sent, ownership_key.as_ref());
            }
        };
        let (execute, existing_result) = match self.command_journal.begin(&command_id, canonical) {
            Ok(journal) => journal,
            Err(error) => {
                let reply = self.make_command_ack(command_id, Err(error));
                let sent = self.send_outbound(reply).await.is_ok();
                return self.finish_command_after_reply(sent, ownership_key.as_ref());
            }
        };
        if let Some(pending) = self.full_snapshot_command.as_ref() {
            if pending.command_id() == command_id {
                // 同一 durable lifecycle 的重投帧只合并；keyed ledger 已消费 replay
                // queued token，但保留原 processing owner，直到有序 Goodbye/EOF 确认。
                return match pending {
                    FullSnapshotCommandState::ReplyReady { .. } => {
                        let reply = self.make_command_ack(command_id, Ok(()));
                        if self.send_outbound(reply).await.is_ok() {
                            self.reply_sent_this_session = true;
                        }
                        false
                    }
                    FullSnapshotCommandState::PendingChunks { .. } => true,
                };
            }
            let reply = self.make_command_ack(
                command_id,
                Err(format!(
                    "full snapshot command {} is still pending",
                    pending.command_id()
                )),
            );
            let sent = self.send_outbound(reply).await.is_ok();
            return self.finish_command_after_reply(sent, ownership_key.as_ref());
        }
        if !execute {
            let result = existing_result
                .unwrap_or_else(|| Err("full snapshot command is already executing".to_owned()));
            let reply = self.make_command_ack(command_id, result);
            let sent = self.send_outbound(reply).await.is_ok();
            return self.finish_command_after_reply(sent, ownership_key.as_ref());
        }
        let batches = self
            .command_executor
            .as_ref()
            .ok_or_else(|| "datasource registry is not configured".to_owned())
            .and_then(|executor| executor.full_snapshot().map_err(|error| error.to_string()));
        let batches = match batches {
            Ok(batches) => batches,
            Err(error) => {
                let result = Err(error);
                self.command_journal.complete(&command_id, result.clone());
                let reply = self.make_command_ack(command_id, result);
                let sent = self.send_outbound(reply).await.is_ok();
                return self.finish_command_after_reply(sent, ownership_key.as_ref());
            }
        };
        if let Some(progress) = &self.progress {
            if let Err(error) = progress.queue_batches_front(batches.len()) {
                tracing::error!(%error, "failed to own authoritative command batches");
                return false;
            }
        }
        self.full_snapshot_command = Some(FullSnapshotCommandState::PendingChunks {
            command_id: command_id.clone(),
        });
        self.initial_batches.extend(batches);
        let advanced = self.advance_full_snapshot_command().await;
        advanced && !self.reply_confirmation_ready()
    }

    pub(crate) async fn advance_full_snapshot_command(&mut self) -> bool {
        let Some(state) = self.full_snapshot_command.clone() else {
            return true;
        };
        let command_id = state.command_id().to_owned();
        if matches!(state, FullSnapshotCommandState::PendingChunks { .. }) {
            if self.drain_authoritative_batches().await.is_err()
                || self.wait_for_pending_frames_empty().await.is_err()
            {
                return false;
            }
            self.full_snapshot_command = Some(FullSnapshotCommandState::ReplyReady {
                command_id: command_id.clone(),
            });
            self.command_journal.complete(&command_id, Ok(()));
        }
        let reply = self.make_command_ack(command_id, Ok(()));
        if self.send_outbound(reply).await.is_err() {
            return false;
        }
        self.reply_sent_this_session = true;
        true
    }

    pub(crate) fn reply_confirmation_ready(&self) -> bool {
        self.reply_sent_this_session
            && matches!(
                self.full_snapshot_command,
                Some(FullSnapshotCommandState::ReplyReady { .. })
            )
    }

    pub(crate) fn make_command_ack(
        &self,
        command_id: String,
        result: Result<(), String>,
    ) -> ClientFrame {
        ClientFrame {
            payload: Some(client_frame::Payload::CommandAck(
                crate::protocol::CommandAck {
                    header: Some(self.make_header(0)),
                    command_id,
                    success: result.is_ok(),
                    error_message: result.err().unwrap_or_default(),
                },
            )),
        }
    }

    pub(crate) async fn drain_authoritative_batches(&mut self) -> Result<(), PushWorkerError> {
        if !self.initial_batches.is_empty() {
            let remaining = self.initial_batches.len();
            if self.snapshot_transaction.is_none() {
                self.start_snapshot_transaction(remaining)?;
            } else if self.snapshot_transaction.as_ref().is_some_and(
                |(_, next_chunk_index, chunk_count)| {
                    chunk_count.saturating_sub(*next_chunk_index) as usize != remaining
                },
            ) {
                return Err(PushWorkerError::InvalidSnapshotTransaction(
                    "restored snapshot transaction does not match remaining chunks".to_owned(),
                ));
            }
        }
        while !self.initial_batches.is_empty() {
            self.wait_for_window_capacity().await?;
            let batch = self
                .initial_batches
                .pop_front()
                .expect("authoritative batch queue was checked as non-empty");
            let retry_batch = batch.clone();
            let pending_before = self.window.len();
            if let Err(error) = self.push_new_batch(batch, None).await {
                if self.window.len() == pending_before {
                    self.initial_batches.push_front(retry_batch);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub(crate) fn start_snapshot_transaction(
        &mut self,
        chunk_count: usize,
    ) -> Result<(), PushWorkerError> {
        let chunk_count = u32::try_from(chunk_count).map_err(|_| {
            PushWorkerError::InvalidSnapshotTransaction(
                "snapshot transaction has too many chunks".to_owned(),
            )
        })?;
        if chunk_count == 0 {
            return Err(PushWorkerError::InvalidSnapshotTransaction(
                "snapshot transaction must have at least one chunk".to_owned(),
            ));
        }
        let first_sequence = self.window.next_sequence().ok_or_else(|| {
            PushWorkerError::InvalidSnapshotTransaction(
                "snapshot transaction cannot allocate a sequence".to_owned(),
            )
        })?;
        self.snapshot_transaction = Some((
            format!("{}-{first_sequence}", self.stream_epoch),
            0,
            chunk_count,
        ));
        Ok(())
    }

    pub(crate) async fn drain_preserved_live_events(&mut self) -> Result<(), PushWorkerError> {
        while !self.preserved_live_events.is_empty() {
            self.wait_for_window_capacity().await?;
            let event = self
                .preserved_live_events
                .pop_front()
                .expect("preserved live queue was checked as non-empty");
            let retry_event = event.clone();
            let pending_before = self.window.len();
            if let Err(error) = self.process_push_event(event).await {
                if self.window.len() == pending_before {
                    self.preserved_live_events.push_front(retry_event);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub(crate) async fn wait_for_pending_frames_empty(&mut self) -> Result<(), PushWorkerError> {
        use crate::protocol::server_frame::Payload;

        while !self.window.is_empty() {
            self.wait_for_window_capacity().await?;
            if !self.window.is_empty() {
                let frame = tokio::select! {
                    biased;
                    () = self.cancellation.cancelled() => return Err(PushWorkerError::Cancelled),
                    frame = self.transport_rx.recv() => frame.ok_or(PushWorkerError::TransportClosed)?,
                };
                match frame.payload {
                    Some(Payload::BatchAck(ack)) => {
                        self.acknowledge(ack.accepted_sequence)?;
                    }
                    Some(Payload::ResyncRequired(resync)) => {
                        self.resync_required = true;
                        return Err(PushWorkerError::ServerInterrupted(resync.reason));
                    }
                    Some(Payload::Error(error)) if error.close_stream => {
                        return Err(PushWorkerError::ServerInterrupted(error.message));
                    }
                    Some(Payload::Command(command)) => {
                        let key = self.begin_command_ownership(&command)?;
                        let reply = self.make_command_ack(
                            command.command_id,
                            Err("authoritative full snapshot is waiting for ACKs".to_owned()),
                        );
                        self.send_outbound(reply).await?;
                        self.finish_command_ownership(key.as_ref())?;
                    }
                    Some(Payload::HelloAck(_) | Payload::Error(_)) | None => {}
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn wait_for_window_capacity(&mut self) -> Result<(), PushWorkerError> {
        use crate::protocol::server_frame::Payload;

        while self.window.is_full() {
            let frame = tokio::select! {
                biased;
                () = self.cancellation.cancelled() => return Err(PushWorkerError::Cancelled),
                frame = self.transport_rx.recv() => frame.ok_or(PushWorkerError::TransportClosed)?,
            };

            match frame.payload {
                Some(Payload::BatchAck(ack)) => {
                    self.acknowledge(ack.accepted_sequence)?;
                }
                Some(Payload::Command(command)) => {
                    let key = self.begin_command_ownership(&command)?;
                    let reply = self.make_command_ack(
                        command.command_id,
                        Err("full snapshot command is waiting for ACK capacity".to_owned()),
                    );
                    self.send_outbound(reply).await?;
                    self.finish_command_ownership(key.as_ref())?;
                }
                Some(Payload::Error(error)) if error.close_stream => {
                    return Err(PushWorkerError::ServerInterrupted(error.message));
                }
                Some(Payload::ResyncRequired(resync)) => {
                    self.resync_required = true;
                    return Err(PushWorkerError::ServerInterrupted(resync.reason));
                }
                Some(Payload::HelloAck(_) | Payload::Error(_)) | None => {}
            }
        }
        Ok(())
    }

    pub(crate) fn make_header(&self, sequence: u64) -> FrameHeader {
        FrameHeader {
            protocol_version: 1,
            service_name: self.service_name.clone(),
            instance_id: self.instance_id.clone(),
            boot_id: self.boot_id.clone(),
            stream_epoch: self.stream_epoch,
            sequence,
            emitted_at_unix_ms: now_unix_ms(),
        }
    }

    pub(crate) fn acknowledge(&mut self, sequence: u64) -> Result<(), PushWorkerError> {
        self.window.ack(sequence).map_err(PushWorkerError::Window)?;
        if let Some(progress) = &self.progress {
            progress.finish_pending_frame().map_err(ownership_error)?;
        }
        Ok(())
    }
}
