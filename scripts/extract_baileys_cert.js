#!/usr/bin/env node

/**
 * Script para extrair a chave pública WA_CERT_ISSUER do Baileys
 * 
 * Uso:
 *   node scripts/extract_baileys_cert.js /path/to/baileys
 */

const fs = require('fs');
const path = require('path');

function findCertKey(dir) {
  const patterns = [
    /WA_CERT_ISSUER/,
    /NOISE_CERT_DETAILS/,
    /CERT_ISSUER/,
    /trustedRoot/,
    /WA_WEB_PUBLIC_KEY/,
  ];

  const searchFiles = [
    'src/Utils/crypto.ts',
    'src/Utils/crypto.js',
    'src/Socket/noise-handler.ts',
    'src/Socket/noise-handler.js',
    'lib/Utils/crypto.js',
    'lib/Socket/noise-handler.js',
  ];

  for (const file of searchFiles) {
    const fullPath = path.join(dir, file);
    if (!fs.existsSync(fullPath)) continue;

    const content = fs.readFileSync(fullPath, 'utf8');
    
    for (const pattern of patterns) {
      if (pattern.test(content)) {
        console.log(`\n✅ Encontrado em: ${file}\n`);
        
        // Extrair contexto ao redor do match
        const lines = content.split('\n');
        let startLine = -1;
        
        for (let i = 0; i < lines.length; i++) {
          if (pattern.test(lines[i])) {
            startLine = i;
            break;
          }
        }
        
        if (startLine >= 0) {
          const context = lines.slice(
            Math.max(0, startLine - 2),
            Math.min(lines.length, startLine + 35)
          ).join('\n');
          
          console.log('Contexto:\n');
          console.log(context);
          console.log('\n' + '='.repeat(80));
          
          // Tentar extrair Buffer/Array
          const bufferMatch = content.match(/Buffer\.from\(\[([\s\S]*?)\]\)/);
          if (bufferMatch) {
            const bytes = bufferMatch[1]
              .split(',')
              .map(s => s.trim())
              .filter(s => s.match(/0x[0-9a-fA-F]{2}/))
              .map(s => parseInt(s, 16));
            
            if (bytes.length === 32) {
              const hex = Buffer.from(bytes).toString('hex');
              console.log('\n🎯 CHAVE EXTRAÍDA (hex 64 chars):\n');
              console.log(hex);
              console.log('\n📋 Use esta env:\n');
              console.log(`export WA_NOISE_CERT_ISSUER_KEYS="${hex}"`);
              return true;
            }
          }
        }
      }
    }
  }
  
  return false;
}

const baileysDirArg = process.argv[2];

if (!baileysDirArg) {
  console.error('❌ Uso: node extract_baileys_cert.js /path/to/baileys');
  process.exit(1);
}

const baileysDir = path.resolve(baileysDirArg);

if (!fs.existsSync(baileysDir)) {
  console.error(`❌ Diretório não encontrado: ${baileysDir}`);
  process.exit(1);
}

console.log(`🔍 Procurando chave WA_CERT_ISSUER em: ${baileysDir}\n`);

if (!findCertKey(baileysDir)) {
  console.log('\n⚠️  Chave não encontrada automaticamente.');
  console.log('\nProcure manualmente por:');
  console.log('  - WA_CERT_ISSUER');
  console.log('  - NOISE_CERT_DETAILS');
  console.log('  - Buffer.from([0x..., 0x..., ...]) com 32 bytes');
  console.log('\nEm arquivos como:');
  console.log('  - src/Utils/crypto.ts');
  console.log('  - src/Socket/noise-handler.ts');
}
