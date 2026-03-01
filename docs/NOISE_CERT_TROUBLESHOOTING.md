# Noise Certificate Troubleshooting

## Erro: "noise intermediate certificate signature invalid"

### Sintoma

```
handshake failed at ServerHello:
noise intermediate certificate signature invalid (trusted_issuer_keys=1)
```

### Causa

O WhatsApp Web atualizou a chave pública root usada para assinar certificados intermediários no handshake Noise XX.

### Solução

#### 1. Obter a chave pública atual do Baileys

No repositório do Baileys, procure por:
- `WA_CERT_ISSUER` ou `NOISE_CERT_DETAILS` 
- Normalmente em `src/Utils/crypto.ts` ou `src/Socket/noise-handler.ts`

Exemplo de como pode aparecer:
```typescript
export const WA_CERT_DETAILS = {
  SERIAL: 0,
  ISSUER: {
    KEY: Buffer.from([
      0xAB, 0x12, 0xCD, 0x34, // ... 32 bytes total
    ])
  }
}
```

#### 2. Converter para hex

Se a chave estiver em Buffer/Array, converta para hex de 64 caracteres:
```javascript
// Node.js
Buffer.from([0xAB, 0x12, ...]).toString('hex')
// Resultado: "ab12cd34..."
```

#### 3. Configurar múltiplas chaves

Adicione a nova chave mantendo a antiga para compatibilidade:

```bash
export WA_NOISE_CERT_ISSUER_KEYS="142375574d0a587166aae71ebe516437c4a28b73e3695c6ce1f7f9545da8ee6b,<NOVA_CHAVE_HEX_64_CHARS>"
```

#### 4. Reiniciar o serviço

```bash
cargo run
```

### Verificação

Se a configuração estiver correta, você verá no log:
```
handshake completed successfully
```

### Notas técnicas

- O código em `src/wa/noise_md.rs` já suporta múltiplas chaves via `WA_NOISE_CERT_ISSUER_KEYS`
- A função `trusted_issuer_keys()` tenta todas as chaves configuradas
- Se nenhuma env estiver configurada, usa a chave hardcoded como fallback
- A validação ocorre ANTES de decodificar os detalhes do certificado (como no Baileys)

### Chave hardcoded atual (fallback)

```
142375574d0a587166aae71ebe516437c4a28b73e3695c6ce1f7f9545da8ee6b
```

Esta chave pode estar desatualizada dependendo da versão do WA Web.
