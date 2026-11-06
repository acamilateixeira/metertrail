# Wiki do MeterTrail

**O MeterTrail conta o uso de uma API uma única vez por evento, mesmo quando o cliente reenvia.** As páginas detalhadas do wiki estão em inglês. Esta página resume o projeto e indica onde ler cada assunto.

[English](Home)

## Resumo

- **O problema:** depois de um timeout, o cliente reenvia o evento. Se a API contar de novo, o cliente paga duas vezes pelo mesmo uso.
- **A garantia:** cada `event_id` conta no máximo uma vez por projeto. O mesmo `event_id` com conteúdo diferente devolve `409`, porque não dá para saber qual versão vale.
- **O isolamento:** a chave da API identifica o projeto no servidor. O mesmo `event_id` em outro projeto é outro evento.
- **A recuperação:** a projeção copia o ledger para o analytics em lotes. Se o processo cair depois de escrever no analytics e antes de gravar o checkpoint, o lote é escrito de novo e o total não muda, porque o uso conta IDs distintos.
- **A prova:** um teste dispara 16 requisições simultâneas do mesmo evento, outros testes injetam a falha e reprocessam tudo, e um script confere as quatro garantias contra o servidor rodando. O CI executa tudo a cada mudança.

## Onde ler

| Assunto | Página |
| --- | --- |
| Rodar e ver as garantias | [Getting Started](Getting-Started) |
| Requisições e respostas | [API Reference](API-Reference) |
| Por que reenvio não conta duas vezes | [Concepts](Concepts) |
| Arquitetura atual e plano em nuvem | [Architecture](Architecture) |
| Decisões de projeto | [Design Decisions](Design-Decisions) |
| Testes e demo | [Testing and Demo](Testing-and-Demo) |
| Próximos marcos | [Roadmap](Roadmap) |

O [README em português](https://github.com/acamilateixeira/metertrail/blob/main/README.pt-BR.md) tem o início rápido completo.
