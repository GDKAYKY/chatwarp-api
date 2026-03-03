    use super::*;
    use warp_core::aes_gcm::{Aes256Gcm, KeyInit};

    #[tokio::test]
    async fn test_encrypt_and_send_returns_both_buffers() {
        // Create a mock transport
        let transport = Arc::new(crate::transport::mock::MockTransport);

        // Create dummy keys for testing
        let key = [0u8; 32];
        let write_key =
            Aes256Gcm::new_from_slice(&key).expect("32-byte key should be valid for AES-256-GCM");
        let read_key =
            Aes256Gcm::new_from_slice(&key).expect("32-byte key should be valid for AES-256-GCM");

        let socket = NoiseSocket::new(transport, write_key, read_key);

        // Create buffers with some initial capacity
        let plaintext_buf = Vec::with_capacity(1024);
        let encrypted_buf = Vec::with_capacity(1024);

        // Store the capacities for verification
        let plaintext_capacity = plaintext_buf.capacity();
        let encrypted_capacity = encrypted_buf.capacity();

        // Call encrypt_and_send - this should return both buffers
        let result = socket.encrypt_and_send(plaintext_buf, encrypted_buf).await;

        assert!(result.is_ok(), "encrypt_and_send should succeed");

        let (returned_plaintext, returned_encrypted) =
            result.expect("encrypt_and_send result should unwrap after is_ok check");

        // Verify both buffers are returned
        assert_eq!(
            returned_plaintext.capacity(),
            plaintext_capacity,
            "Plaintext buffer should maintain its capacity"
        );
        assert_eq!(
            returned_encrypted.capacity(),
            encrypted_capacity,
            "Encrypted buffer should maintain its capacity"
        );

        // Verify buffers are cleared
        assert!(
            returned_plaintext.is_empty(),
            "Returned plaintext buffer should be cleared"
        );
        assert!(
            returned_encrypted.is_empty(),
            "Returned encrypted buffer should be cleared"
        );
    }

    #[tokio::test]
    async fn test_concurrent_sends_maintain_order() {
        use async_trait::async_trait;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        // Create a mock transport that records the order of sends by decrypting
        // the first byte (which contains the task index)
        struct RecordingTransport {
            recorded_order: Arc<Mutex<Vec<u8>>>,
            read_key: Aes256Gcm,
            counter: std::sync::atomic::AtomicU32,
        }

        #[async_trait]
        impl crate::transport::Transport for RecordingTransport {
            async fn send(&self, data: &[u8]) -> std::result::Result<(), anyhow::Error> {
                // Decrypt the data to extract the index (first byte of plaintext)
                if data.len() > 16 {
                    // Skip the noise frame header (3 bytes for length)
                    let ciphertext = &data[3..];
                    let counter = self
                        .counter
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let iv = super::generate_iv(counter);

                    if let Ok(plaintext) = self.read_key.decrypt(iv.as_ref().into(), ciphertext)
                        && !plaintext.is_empty()
                    {
                        let index = plaintext[0];
                        let mut order = self.recorded_order.lock().await;
                        order.push(index);
                    }
                }
                Ok(())
            }

            async fn disconnect(&self) {}
        }

        let recorded_order = Arc::new(Mutex::new(Vec::new()));
        let key = [0u8; 32];
        let write_key =
            Aes256Gcm::new_from_slice(&key).expect("32-byte key should be valid for AES-256-GCM");
        let read_key =
            Aes256Gcm::new_from_slice(&key).expect("32-byte key should be valid for AES-256-GCM");

        let transport = Arc::new(RecordingTransport {
            recorded_order: recorded_order.clone(),
            read_key: Aes256Gcm::new_from_slice(&key)
                .expect("32-byte key should be valid for AES-256-GCM"),
            counter: std::sync::atomic::AtomicU32::new(0),
        });

        let socket = Arc::new(NoiseSocket::new(transport, write_key, read_key));

        // Spawn multiple concurrent sends with their indices
        let mut handles = Vec::new();
        for i in 0..10 {
            let socket = socket.clone();
            handles.push(tokio::spawn(async move {
                // Use index as the first byte of plaintext to identify this send
                let mut plaintext = vec![i as u8];
                plaintext.extend_from_slice(&[0u8; 99]);
                let out_buf = Vec::with_capacity(256);
                socket.encrypt_and_send(plaintext, out_buf).await
            }));
        }

        // Wait for all sends to complete
        for handle in handles {
            let result = handle.await.expect("task should complete");
            assert!(result.is_ok(), "All sends should succeed");
        }

        // Verify all sends completed in FIFO order (0, 1, 2, ..., 9)
        let order = recorded_order.lock().await;
        let expected: Vec<u8> = (0..10).collect();
        assert_eq!(*order, expected, "Sends should maintain FIFO order");
    }
