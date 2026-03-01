# Noise Certificate Troubleshooting

## Erro: falha na validação do certificado Noise

### Sintoma

```
handshake failed at ServerHello:
noise leaf certificate key does not match server static key
```

### Causa

A validação de certificado Noise falha quando o certificado informado pelo servidor
não bate com a chave estática descriptografada durante o handshake. Isso normalmente
indica uma mudança de formato/protocolo do WhatsApp Web e exige atualização do cliente.

### Solução

#### 1. Atualizar o binário

Certifique-se de estar rodando a versão mais recente do `chatwarp-api`
compilada a partir do `main` atualizado.

### Verificação

Se a configuração estiver correta, você verá no log:
```
handshake completed successfully
```

### Notas técnicas

Atualmente a validação de certificado segue a mesma lógica do crate `whatsapp-rust`:

- O certificado intermediário é validado apenas de forma estrutural:
  - `issuer_serial` deve ser igual a `0`
  - A chave pública (`key`) deve ter 32 bytes
- O certificado leaf é validado checando:
  - `issuer_serial` do leaf deve apontar para o `serial` do intermediário
  - A chave pública (`key`) do leaf deve ser **idêntica** à chave estática descriptografada no handshake
- Campos de `signature` dos certificados não são mais utilizados na validação.

Isso evita dependência de chaves root hardcoded e acompanha o comportamento atual do `whatsapp-rust`/Baileys.
