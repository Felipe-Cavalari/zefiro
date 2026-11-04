# Política de segurança

## Versões com suporte

Só a **última release** recebe correções. O Zefiro avisa no menu do Z quando há versão nova.

## Como reportar uma vulnerabilidade

**Não abra uma issue pública.** Use o relato privado do GitHub:
[Security → Report a vulnerability](https://github.com/Felipe-Cavalari/zefiro/security/advisories/new).

Conte o que você encontrou, como reproduzir e o impacto que imagina. Eu respondo assim que puder (é um projeto de uma pessoa só, então pode levar alguns dias) e aviso quando a correção sair.

## O que interessa em especial

O Zefiro roda no seu usuário, sem administrador, e lida com:

- tokens do Spotify (cifrados pelo DPAPI em `%LOCALAPPDATA%\zefiro\spotify`);
- conversas do chat (arquivos em `%LOCALAPPDATA%\zefiro\chat`, que nunca vão para o log);
- a atualização automática, que só baixa o instalador de `github.com/Felipe-Cavalari/zefiro/releases` e confere o SHA-256 (ADR-0014);
- processos filhos (`claude`, `codex`, `grok`, `wt.exe`, `wsl.exe`) e leitura de pastas do WSL.

Não há telemetria: nada sai do PC sem você pedir (chat, Spotify, clima, atualização).

## Fora do escopo

- Problemas que exigem acesso de administrador ou já ter controle da sua conta do Windows.
- Os termos de uso das assinaturas do Claude e do Grok, descritos nos avisos do README.
