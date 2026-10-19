# Emerald

Cliente de Discord nativo para macOS, escrito em Rust com [gpui](https://www.gpui.rs/).
A interface segue o visual do cliente web em pt-BR, e o app gasta uma fração da memória
de um Electron: algo entre 60 e 100 MB e CPU perto de zero em repouso.

> **Aviso.** Clientes de terceiros com conta de usuário violam os Termos de Serviço do
> Discord e podem levar à suspensão da conta. O Emerald imita o cliente web oficial e
> respeita os limites de requisição, mas o risco continua sendo seu. Use por conta própria.

## O que já funciona

- **Login** pela página oficial do Discord, num WKWebView nativo, com suporte a captcha.
  O token fica em `~/.config/emerald/token`; sair revoga o token no servidor.
- **Amigos**: abas Disponível, Todos e Pendente, adicionar por nome de usuário, aceitar,
  recusar e remover.
- **Mensagens diretas**: lista ordenada por atividade, busca, histórico, envio com
  confirmação otimista, "digitando…", painel de perfil (selos, bio, contas conectadas)
  e o perfil completo.
- **Servidores**: canais e categorias recolhíveis, permissões calculadas como no Discord
  (canais ocultos, envio bloqueado sem permissão), apelidos e cores de cargo, respostas,
  imagens, embeds, figurinhas e mensagens de sistema.
- **Markdown do Discord**: negrito, itálico, sublinhado, tachado, código, títulos,
  menções, links clicáveis. Links mascarados sempre mostram o domínio real.
- **Lista de membros** carregada sob demanda: mesmo em servidores com milhões de
  membros, só as páginas visíveis ficam na memória.
- **Não lidos** sincronizados com os outros dispositivos e recibos de leitura agrupados.
- **Configurações**: nome exibido, status, tamanho da fonte, modo compacto.

Ainda não há voz, threads, fóruns, reações, emoji como imagem nem edição de mensagens.

## Requisitos

- macOS (o login usa WebKit e AppKit).
- Rust estável recente (edição 2024).
- Não precisa do Xcode completo: os shaders Metal são compilados em tempo de execução
  (`gpui` com `runtime_shaders`).

## Rodando

```sh
cargo run --release
```

Na primeira execução abre a janela de login do Discord. Para usar um token que você já tem:

```sh
DISCORD_TOKEN=... cargo run --release
```

Variáveis úteis:

| Variável | Para quê |
|---|---|
| `DISCORD_TOKEN` | Usa este token em vez do salvo. |
| `EMERALD_BUILD_NUMBER` | Substitui o `client_build_number` quando o do código envelhece. |
| `RUST_LOG=emerald=debug` | Logs detalhados. |

Testes:

```sh
cargo test
```

## Arquivos

| Caminho | Conteúdo |
|---|---|
| `~/.config/emerald/token` | Token da conta. Nunca compartilhe. |
| `~/.config/emerald/settings.json` | Preferências. |
| `~/Library/Caches/emerald/cdn` | Cache de imagens do CDN, limitado a 200 MB. |

## Arquitetura

Duas threads: a da interface (gpui) e uma de rede com um runtime tokio de uma thread só.
Elas conversam por mensagens: a UI manda `Command`s, a rede devolve `NetEvent`s.

```
src/
├── main.rs              ponto de entrada, modos filho do webview, ciclo de login
├── settings.rs          preferências em JSON
├── webview.rs           WKWebView de login e captcha (processo filho)
├── discord/             tudo que fala com o Discord, sem nada de interface
│   ├── protocol.rs      Command, NetEvent, Snapshot: o contrato UI ↔ rede
│   ├── commands.rs      executa cada Command na API REST
│   ├── gateway.rs       WebSocket com zlib-stream, resume, heartbeat, ops 8 e 37
│   ├── rest/            cliente HTTP: builder `Call`, RateLimiter, erros tipados
│   ├── model/           payloads do Discord, só com os campos usados
│   ├── permissions.rs   algoritmo de permissões por cargo e sobrescrita
│   ├── identity.rs      cabeçalhos e super properties iguais aos do web
│   └── cache.rs         cache de imagens em disco
└── ui/
    ├── markdown.rs      parser do markdown do Discord
    ├── text_input.rs    campo de texto
    ├── theme.rs, icons.rs, time.rs
    └── app/             a janela principal
        ├── app.rs       `Emerald`: compõe os stores e desenha o layout
        ├── state/       stores sem interface, testados isoladamente
        ├── events.rs    distribui cada NetEvent aos stores
        ├── navigation.rs, composer.rs
        └── sidebar.rs, chat/, members.rs, friends.rs, profile.rs, settings.rs, …
```

Algumas decisões:

- **Stores sem interface.** `People`, `Spaces`, `History`, `ReadStates`, `Typing` e
  `MemberList` guardam o estado e não sabem desenhar. A lista de mensagens na tela
  acompanha o `History` pela trait `MessageView`, e a UI envia comandos pela trait
  `Outbox`. Os dois lados têm implementações falsas nos testes.
- **Comportamento de navegador.** Cabeçalhos, nonces, recibos de leitura, ops 8 e 37
  e a paginação seguem o que o cliente web faz, para não destoar nem abusar da API.
  O REST respeita os buckets de rate limit, um teto global abaixo do do Discord e uma
  trava contra excesso de requisições inválidas.
- **Memória limitada.** Histórico de até 8 canais com 300 mensagens cada, até 1500
  imagens decodificadas e só as páginas visíveis da lista de membros.
