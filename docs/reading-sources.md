# Computing, infrastructure, security and AI

The bundled selection contains **98 subscriptions and 46 smart lists**:
64 computing sources, 12 additional platform/kernel/network sources, eight
security-research sources and 14 artificial-intelligence sources. All bundled names, descriptions, topic
labels and product guides are in English. Publisher content and personal
notes keep their original language.

## Open and collect

After `make build`, open the workspace library with:

```sh
./lume
./lume feeds
./lume sync
./lume --offline
```

The launcher prepares `.local/library.sqlite3` and upgrades its selection once.
The TUI collects in the background on startup. A newly installed subscription
has no articles until a collection succeeds. Offline mode reads the cache.

This library is separate from the default database in `$XDG_DATA_HOME/lume`
or `~/.local/share/lume`. Install the selection there with
`./target/release/lume starter`, or choose another database using
`./target/release/lume --database PATH starter`. Installation preserves cached
articles, personal folders, favorites, annotations and existing query criteria.
It re-enables selected subscriptions that were previously disabled.

The [computing](../assets/reading/computing.opml),
[security](../assets/reading/security.opml) and
[AI](../assets/reading/ai.opml), and
[infrastructure](../assets/reading/infrastructure.opml) OPML assets can also be imported elsewhere.
Their groups organize subscription files; they do not create article folders.

## Your reading lists

**19 · Security** automatically includes every cached article from its eight
selected sources. **20 · Artificial intelligence** includes all articles from
its 14 selected sources. Both groups include future publications, without
requiring keywords. Existing lists with these names keep their own criteria.
The `:smart` form preserves a group's source membership while editing its
other criteria.

The 40 topic smart lists use literal full-text prefixes across the entire
library. The ten new AI searches are:

| List | Literal text |
| --- | --- |
| 21 · Transformers | transformer |
| 22 · LLMs | llm |
| 23 · Model training | training |
| 24 · Fine-tuning | fine-tuning |
| 25 · RAG and retrieval | retrieval |
| 26 · Inference and serving | inference |
| 27 · Model evaluation | evaluation |
| 28 · AI agents | agent |
| 29 · Multimodal models | multimodal |
| 30 · AI infrastructure | gpu |

These are discovery queries, not semantic classifications. An article using
only “attention” may not match Transformers; broad words may include unrelated
articles. Edit terms and source/age/unread criteria with `:smart`. Use the
source groups for complete coverage of the selected publishers.

Historical bundled Portuguese labels become English in place, preserving
saved-search IDs, active views and customized criteria. If an English name
already exists, both queries remain, with an `(imported)` suffix on the
upgraded historical list. Other personal names and publisher text are retained.

## Platforms, kernels and networks

The four infrastructure collections combine new subscriptions with relevant
sources already in your library. The same source can appear in several lists
without creating another subscription or copying an article.

| Collection | Sources | Focus |
| --- | ---: | --- |
| 31 · Platform engineering | 8 | Developer portals, control planes, GitOps, cost allocation and platform workflows |
| 32 · Kernel and systems | 7 | Drivers, kernel internals, immutable hosts, namespaces, runtimes and debugging |
| 33 · Networks and dataplanes | 8 | Cilium, eBPF, Internet routing, overlays, service meshes and network operations |
| 34 · DevOps and delivery | 8 | CI/CD, build provenance, deployment engineering, reliability and operations |

These are publisher collections: all articles from their selected sources
appear, including ones outside the collection's main focus. Existing custom
criteria remain intact. This is useful for discovering mechanisms before you
know which keyword to search for.

Twelve additional literal searches provide narrower entry points:

| List | Literal text |
| --- | --- |
| 35 · eBPF and tracing | ebpf |
| 36 · Cilium and Hubble | cilium |
| 37 · CNI and pod networking | cni |
| 38 · Network policy | network policy |
| 39 · Container internals | container |
| 40 · Infrastructure as code | terraform |
| 41 · Kernel programming | kernel |
| 42 · Routing and BGP | bgp |
| 43 · Kubernetes controllers | controller |
| 44 · Storage and filesystems | filesystem |
| 45 · Scheduling and resources | scheduler |
| 46 · Software supply chain | supply chain |

Search words combine with AND, as elsewhere in Lume. These labels describe
reading interests rather than synonyms: a Hubble-only article may not match
`cilium`, and an OpenTofu-only article may not match `terraform`. Browse the
source collections for broader coverage or edit criteria with `:smart`.

For a practical learning path, trace a request from a process through its
network namespace, veth pair, bridge or eBPF dataplane, CNI configuration,
network policy and service routing. Then compare the control loop configuring
that path with the data structures forwarding packets. In parallel, explore
how developer portals and infrastructure control planes expose those building
blocks to application teams. Bootlin and kernel maintainer posts cover the
lower layers; iximiuz covers container mechanisms; Cilium and eBPF.io connect
them to cloud-native systems. APNIC and ipSpace add the routing perspective.

The twelve new endpoints were researched on 2026-10-17. Eleven returned XML,
RSS or Atom content types to the research browser. Tekton's blog feed returned
an opaque HTTP-OK tool response, so its item parsing remains unverified.
Bootlin's RSS link and the kernel community's
[official feed documentation](https://korg.docs.kernel.org/people.html)
identify those publisher endpoints. As with the AI sources, these checks are
not a completed fetch/parse/import qualification. Direct collection remains
subject to the environment's DNS restrictions.

## Organize a reading routine

Type `:` to discover commands and Tab to complete. Create personal folders
with `:folder Study`, `:folder Reference` or `:folder Experiments`. Use `a`
to organize an article into several folders, or `:save Study` to create a
folder if needed and save the current article. `N` also works with no cached
articles. Folders are flat and store articles rather than subscriptions.

Use `f` for Favorites, `m` for read/unread, and `:read` for the automatic Read
folder. `u` undoes a personal change. In the reader, select with `v`, copy with
`y`, save a highlight with `s`, or annotate the selection with `n`.

Try one mechanism, production incident or architecture decision per session.
For AI, start with Jay Alammar or Lilian Weng for explanations, then explore
implementation material from Hugging Face, PyTorch and Sebastian Raschka.
Use the research feeds and arXiv for deeper follow-up. The combined arXiv feed
is high volume and supplies abstracts, not full papers. Older explainers can
remain useful even when a blog publishes infrequently. Commercial blogs also
carry product announcements; a subscription is not an endorsement of a product.

## Source qualification

Source selection was researched against official publisher sites and feed
endpoints on **2026-10-17**. For the AI additions, 13 endpoints responded to
the research browser with RSS, Atom or XML content types. This establishes an
XML response, not successful parsing of every item or a completed collection.
The combined arXiv URL follows its
[official multi-category RSS contract](https://info.arxiv.org/help/rss.html)
using `cs.CL+cs.LG+cs.AI`; that combined response could not be opened by the
research browser. Feed categories and result limits are documented there.

The installed sources remain subject to bounded HTTP collection and item
validation. This development shell cannot currently resolve publisher DNS,
and the required loopback HTTP qualification is blocked by socket permissions;
see [validation](validation.md). Failure of one source does not prevent cached
reading or independent source updates. Feeds may move, disappear or provide
only summaries. Large feeds may exceed the current 4 MiB limit.

The security selection retains the previously researched official endpoints.
The ZDI feed was linked by its [official RSS page](https://www.zerodayinitiative.com/rss/).
The Qualys endpoint was not reachable through the research browser. Existing
source qualifications and network limitations are retained in the validation
record; adding a subscription does not claim end-to-end collection passed.

## Included sources

The following tables mirror the bundled OPML, including each publisher's
reading focus and selected endpoint.

### Kubernetes and cloud native

| Source | Reading focus | Feed |
| --- | --- | --- |
| [Kubernetes Blog](https://kubernetes.io/blog/) | Cluster internals, scheduling, storage and Kubernetes operations. | [RSS/Atom](https://kubernetes.io/feed.xml) |
| [CNCF](https://www.cncf.io/blog/) | Cloud-native projects, adoption reports and platform engineering. | [RSS/Atom](https://www.cncf.io/feed/) |
| [Cilium](https://cilium.io/blog/) | Container networking, eBPF, network policies and observability. | [RSS/Atom](https://cilium.io/blog/rss.xml) |
| [Istio](https://istio.io/latest/blog/) | Service meshes, interservice traffic, mTLS and traffic policies. | [RSS/Atom](https://istio.io/latest/feed.xml) |
| [etcd](https://etcd.io/blog/) | Consensus, distributed storage and Kubernetes control-plane infrastructure. | [RSS/Atom](https://etcd.io/blog/index.xml) |
| [AWS Containers](https://aws.amazon.com/blogs/containers/) | Production containers and Kubernetes operations; includes AWS products. | [RSS/Atom](https://aws.amazon.com/blogs/containers/feed/) |
| [Last Week in Kubernetes Development](https://lwkd.info/) | Weekly pull requests, code changes and Kubernetes development. | [RSS/Atom](https://lwkd.info/feed.xml) |

### Architecture and distributed systems

| Source | Reading focus | Feed |
| --- | --- | --- |
| [InfoQ Architecture & Design](https://www.infoq.com/architecture-design/) | Architecture, scalability, patterns and engineering case studies. | [RSS/Atom](https://feed.infoq.com/architecture-design) |
| [Martin Fowler](https://martinfowler.com/) | Architecture patterns, integration, system evolution and engineering practices. | [RSS/Atom](https://martinfowler.com/feed.atom) |
| [Microservices.io](https://microservices.io/) | Microservices patterns, sagas, distributed transactions and decomposition. | [RSS/Atom](https://microservices.io/feed.xml) |
| [Netflix Technology Blog](https://netflixtechblog.com/) | Architecture at scale, internal platforms, data and reliability. | [RSS/Atom](https://netflixtechblog.com/feed) |
| [Slack Engineering](https://slack.engineering/) | Infrastructure, performance, databases and service operations. | [RSS/Atom](https://slack.engineering/feed/) |
| [Dropbox Tech](https://dropbox.tech/) | Storage, distributed systems, performance and infrastructure. | [RSS/Atom](https://dropbox.tech/feed) |
| [Meta Engineering](https://engineering.fb.com/) | Databases, networking, infrastructure and large-scale systems. | [RSS/Atom](https://engineering.fb.com/feed/) |
| [Spotify Engineering](https://engineering.atspotify.com/) | Platforms, backend systems, data and production engineering practices. | [RSS/Atom](https://engineering.atspotify.com/feed/) |
| [AWS Architecture](https://aws.amazon.com/blogs/architecture/) | Architecture decisions, resilience and case studies; includes AWS products. | [RSS/Atom](https://aws.amazon.com/blogs/architecture/feed/) |
| [All Things Distributed](https://www.allthingsdistributed.com/) | Distributed systems, infrastructure decisions and cloud architecture. | [RSS/Atom](https://www.allthingsdistributed.com/atom.xml) |
| [Marc Brooker](https://brooker.co.za/blog/) | Probability, simulation, concurrency, performance and distributed systems. | [RSS/Atom](https://brooker.co.za/blog/rss.xml) |

### Authentication, authorization and security

| Source | Reading focus | Feed |
| --- | --- | --- |
| [Keycloak](https://www.keycloak.org/blog/) | Identity, OpenID Connect, OAuth, federation and Keycloak operations. | [RSS/Atom](https://www.keycloak.org/rss.xml) |
| [Auth0](https://auth0.com/blog/) | Authentication, OAuth, tokens and application security; includes products. | [RSS/Atom](https://auth0.com/blog/rss.xml) |
| [authentik](https://goauthentik.io/blog/) | SSO, identity, authentication and application integration. | [RSS/Atom](https://goauthentik.io/blog/rss.xml) |
| [OpenFGA](https://openfga.dev/blog/) | Fine-grained authorization models, implementation and access relationships. | [RSS/Atom](https://openfga.dev/blog/rss.xml) |
| [OpenSSF](https://openssf.org/blog/) | Software supply chains, dependencies, provenance and project practices. | [RSS/Atom](https://openssf.org/feed/) |
| [AWS Security](https://aws.amazon.com/blogs/security/) | IAM, credentials, certificates and infrastructure security; includes AWS. | [RSS/Atom](https://aws.amazon.com/blogs/security/feed/) |

### Databases

| Source | Reading focus | Feed |
| --- | --- | --- |
| [Planet PostgreSQL](https://planet.postgresql.org/) | PostgreSQL community articles, internals, operations, SQL and performance. | [RSS/Atom](https://planet.postgresql.org/rss20.xml) |
| [Crunchy Data](https://www.crunchydata.com/blog) | PostgreSQL execution plans, extensions, maintenance and performance. | [RSS/Atom](https://www.crunchydata.com/blog/rss.xml) |
| [pgMustard](https://www.pgmustard.com/blog) | EXPLAIN, query optimization, indexes and PostgreSQL diagnostics. | [RSS/Atom](https://www.pgmustard.com/blog?format=rss) |
| [Percona](https://www.percona.com/blog/) | Database operations, replication, performance and observability. | [RSS/Atom](https://www.percona.com/blog/feed/) |
| [ScyllaDB](https://www.scylladb.com/blog/) | Distributed databases, storage, consistency and performance; includes products. | [RSS/Atom](https://www.scylladb.com/feed/) |
| [Neon](https://neon.com/blog) | PostgreSQL compute/storage separation and serverless operations. | [RSS/Atom](https://neon.com/blog/rss.xml) |
| [ClickHouse](https://clickhouse.com/blog) | Columnar engines, query execution and large-scale analytics. | [RSS/Atom](https://clickhouse.com/rss.xml) |
| [AWS Database](https://aws.amazon.com/blogs/database/) | Database migrations, operations and architectures on AWS. | [RSS/Atom](https://aws.amazon.com/blogs/database/feed/) |

### Data, events and pipelines

| Source | Reading focus | Feed |
| --- | --- | --- |
| [Confluent Developer](https://developer.confluent.io/blog/) | Kafka, Flink, schemas, watermarks and event-processing engineering. | [RSS/Atom](https://developer.confluent.io/blog/rss.xml) |
| [Debezium](https://debezium.io/blog/) | CDC, connectors, change replication and event-driven integration. | [RSS/Atom](https://debezium.io/blog.atom) |
| [Apache Airflow](https://airflow.apache.org/blog/) | Orchestration, scheduling, DAGs and pipeline operations. | [RSS/Atom](https://airflow.apache.org/blog/index.xml) |
| [Apache Beam](https://beam.apache.org/blog/) | Batch and stream processing, runners and portable data pipelines. | [RSS/Atom](https://beam.apache.org/feed.xml) |
| [Apache Pulsar](https://pulsar.apache.org/blog/) | Messaging, streaming, topics, storage and cluster operations. | [RSS/Atom](https://pulsar.apache.org/blog/rss.xml) |
| [Materialize](https://materialize.com/blog/) | Incremental computation, streaming SQL and materialized views. | [RSS/Atom](https://materialize.com/rss.xml) |
| [DuckDB](https://duckdb.org/news/) | Analytical SQL, file formats, vectorized execution and data engineering. | [RSS/Atom](https://duckdb.org/feed.xml) |
| [Databricks](https://www.databricks.com/blog) | Spark, lakehouses, Delta and data platforms; includes product announcements. | [RSS/Atom](https://www.databricks.com/feed) |
| [AWS Big Data](https://aws.amazon.com/blogs/big-data/) | Pipelines, streaming, analytical platforms and data operations on AWS. | [RSS/Atom](https://aws.amazon.com/blogs/big-data/feed/) |

### SRE and observability

| Source | Reading focus | Feed |
| --- | --- | --- |
| [SRE Weekly](https://sreweekly.com/) | Curated availability, incidents, automation and reliability articles. | [RSS/Atom](https://sreweekly.com/feed/) |
| [Grafana](https://grafana.com/blog/) | Metrics, logs, traces, alerts and observability operations. | [RSS/Atom](https://grafana.com/blog/index.xml) |
| [OpenTelemetry](https://opentelemetry.io/blog/) | Instrumentation, collection, context propagation and observability signals. | [RSS/Atom](https://opentelemetry.io/blog/index.xml) |
| [Datadog Engineering](https://www.datadoghq.com/blog/engineering/) | Platform engineering, observability internals and performance. | [RSS/Atom](https://www.datadoghq.com/blog/engineering/index.xml) |
| [Charity Majors](https://charity.wtf/) | Observability, operations, incidents and infrastructure team practices. | [RSS/Atom](https://charity.wtf/feed/) |

### Linux, networking and performance

| Source | Reading focus | Feed |
| --- | --- | --- |
| [Cloudflare](https://blog.cloudflare.com/) | Networking, protocols, DNS, TLS, security and global infrastructure. | [RSS/Atom](https://blog.cloudflare.com/rss/) |
| [Tailscale](https://tailscale.com/blog/) | Networking, WireGuard, access and connectivity; includes product news. | [RSS/Atom](https://tailscale.com/blog/index.xml) |
| [Fly.io](https://fly.io/blog/) | Virtualization, networking, application execution and distributed infrastructure. | [RSS/Atom](https://fly.io/blog/feed.xml) |
| [LWN.net](https://lwn.net/) | Linux, kernel and systems software; some articles require a subscription. | [RSS/Atom](https://lwn.net/headlines/rss) |
| [Brendan Gregg](https://www.brendangregg.com/blog/) | Systems performance, profiling, eBPF and resource analysis. | [RSS/Atom](https://www.brendangregg.com/blog/rss.xml) |
| [Julia Evans](https://jvns.ca/) | Linux, networking, tools, DNS and practical explanations of systems mechanisms. | [RSS/Atom](https://jvns.ca/atom.xml) |

### Platforms, CI/CD and infrastructure as code

| Source | Reading focus | Feed |
| --- | --- | --- |
| [GitHub Engineering](https://github.blog/engineering/) | Developer infrastructure, databases, builds and engineering at scale. | [RSS/Atom](https://github.blog/engineering/feed/) |
| [Pulumi](https://www.pulumi.com/blog/) | Infrastructure as code, platform automation and provisioning. | [RSS/Atom](https://www.pulumi.com/blog/rss.xml) |
| [CircleCI](https://circleci.com/blog/) | Delivery pipelines, testing, builds and CI/CD operations. | [RSS/Atom](https://circleci.com/blog/feed.xml) |
| [Docker](https://www.docker.com/blog/) | Containers, images, builds and software supply chains; includes product news. | [RSS/Atom](https://www.docker.com/blog/feed/) |
| [Jenkins](https://www.jenkins.io/blog/) | Automation, pipelines and CI/CD infrastructure operations. | [RSS/Atom](https://www.jenkins.io/rss.xml) |

### Computing foundations and research

| Source | Reading focus | Feed |
| --- | --- | --- |
| [Lobsters](https://lobste.rs/) | Community computing, systems, networking, Linux and technical discussions. | [RSS/Atom](https://lobste.rs/rss) |
| [Dan Luu](https://danluu.com/) | Measurements, latency, systems, performance and technical decision analysis. | [RSS/Atom](https://danluu.com/atom.xml) |
| [Hillel Wayne](https://www.hillelwayne.com/) | Formal methods, specifications, invariants and reasoning about systems. | [RSS/Atom](https://www.hillelwayne.com/index.xml) |
| [Murat Demirbas](https://muratbuffalo.blogspot.com/) | Papers, consensus, replication and distributed-systems research. | [RSS/Atom](https://muratbuffalo.blogspot.com/feeds/posts/default) |
| [Go Blog](https://go.dev/blog/) | Runtime, concurrency, tooling and Go language implementation. | [RSS/Atom](https://go.dev/blog/feed.atom) |
| [Rust Blog](https://blog.rust-lang.org/) | Language, tooling, security and Rust ecosystem development. | [RSS/Atom](https://blog.rust-lang.org/feed.xml) |
| [The Morning Paper — Archive](https://blog.acolyer.org/) | Historical paper analyses; an archive without a regular publishing schedule. | [RSS/Atom](https://blog.acolyer.org/feed/) |

### Security

| Source | Reading focus | Feed |
| --- | --- | --- |
| [Google Project Zero](https://projectzero.google/) | Exploit chains, memory safety, browser and operating-system vulnerabilities. | [RSS/Atom](https://projectzero.google/feed.xml) |
| [PortSwigger Research](https://portswigger.net/research) | Web exploitation, request smuggling, cache poisoning and authentication flaws. | [RSS/Atom](https://portswigger.net/research/rss) |
| [Trail of Bits](https://blog.trailofbits.com/) | Audits, fuzzing, compilers, cryptography and software supply chains. | [RSS/Atom](https://blog.trailofbits.com/index.xml) |
| [Zero Day Initiative](https://www.zerodayinitiative.com/blog) | Vulnerability root causes, Pwn2Own research and patch analysis. | [RSS/Atom](https://www.zerodayinitiative.com/blog?format=rss) |
| [Cisco Talos](https://blog.talosintelligence.com/) | Vulnerability research, malware analysis and incident response. | [RSS/Atom](https://blog.talosintelligence.com/rss/) |
| [Qualys Threat Research](https://blog.qualys.com/vulnerabilities-threat-research) | Linux vulnerabilities, root-cause analysis and remediation. | [RSS/Atom](https://blog.qualys.com/vulnerabilities-threat-research/feed) |
| [GitHub Vulnerability Research](https://github.blog/security/vulnerability-research/) | Open-source vulnerabilities, CodeQL and supply-chain research. | [RSS/Atom](https://github.blog/security/vulnerability-research/feed/) |
| [Google Security Blog](https://security.googleblog.com/) | Security engineering, vulnerability research and mitigations. | [RSS/Atom](https://security.googleblog.com/feeds/posts/default) |

### Artificial intelligence

| Source | Reading focus | Feed |
| --- | --- | --- |
| [Hugging Face](https://huggingface.co/blog) | Transformers, open models, datasets, fine-tuning and inference tutorials. | [RSS/Atom](https://huggingface.co/blog/feed.xml) |
| [Google Research](https://research.google/blog/) | Machine learning, language models, algorithms and research systems. | [RSS/Atom](https://research.google/blog/rss/) |
| [Google DeepMind](https://deepmind.google/blog/) | Foundation models, multimodal learning, reasoning and AI research. | [RSS/Atom](https://deepmind.google/blog/rss.xml) |
| [Berkeley AI Research](https://bair.berkeley.edu/blog/) | Research explainers on learning, robotics, language and generative models. | [RSS/Atom](https://bair.berkeley.edu/blog/feed.xml) |
| [Stanford AI Lab](https://ai.stanford.edu/blog/) | Machine learning, NLP, representation learning and AI research. | [RSS/Atom](https://ai.stanford.edu/blog/feed.xml) |
| [Lilian Weng](https://lilianweng.github.io/) | Detailed explainers on transformers, training, agents and generative models. | [RSS/Atom](https://lilianweng.github.io/index.xml) |
| [Jay Alammar](https://jalammar.github.io/) | Visual explanations of attention, transformers, embeddings and language models. | [RSS/Atom](https://jalammar.github.io/feed.xml) |
| [Sebastian Raschka](https://magazine.sebastianraschka.com/) | LLM architectures, implementation, training, evaluations and paper analysis. | [RSS/Atom](https://magazine.sebastianraschka.com/feed) |
| [The Gradient](https://thegradient.pub/) | Research perspectives, deep learning and critical analysis of AI. | [RSS/Atom](https://thegradient.pub/rss/) |
| [PyTorch](https://pytorch.org/blog/) | Training frameworks, distributed workloads, compilers and model performance. | [RSS/Atom](https://pytorch.org/feed/) |
| [Chip Huyen](https://huyenchip.com/) | AI engineering, evaluation, inference, data and production ML systems. | [RSS/Atom](https://huyenchip.com/feed.xml) |
| [TensorFlow](https://blog.tensorflow.org/) | Model training, deployment, tooling and machine-learning infrastructure. | [RSS/Atom](https://blog.tensorflow.org/feeds/posts/default) |
| [arXiv — AI, NLP and machine learning](https://info.arxiv.org/help/rss.html) | Daily paper abstracts across AI, NLP and ML; high volume, not full papers. | [RSS/Atom](https://rss.arxiv.org/rss/cs.CL+cs.LG+cs.AI) |
| [OpenAI](https://openai.com/news/) | Model releases, research, evaluations and AI systems; includes product news. | [RSS/Atom](https://openai.com/news/rss.xml) |

### Developer platforms and delivery

| Source | Reading focus | Feed |
| --- | --- | --- |
| [Backstage](https://backstage.io/blog/) | Developer portals, software catalogs, platform adoption and engineering workflows. | [RSS/Atom](https://backstage.io/blog/rss.xml) |
| [Crossplane](https://blog.crossplane.io/) | Control planes, compositions, infrastructure APIs and platform engineering. | [RSS/Atom](https://blog.crossplane.io/rss/) |
| [Flux](https://fluxcd.io/blog/) | GitOps reconciliation, delivery automation, tenancy and Kubernetes operations. | [RSS/Atom](https://fluxcd.io/blog/index.xml) |
| [Tekton](https://tekton.dev/blog/) | Cloud-native CI/CD pipelines, build provenance and software delivery. | [RSS/Atom](https://tekton.dev/blog/index.xml) |
| [OpenCost](https://opencost.io/blog/) | Kubernetes cost allocation, resource efficiency and infrastructure economics. | [RSS/Atom](https://opencost.io/blog/rss.xml) |
| [Flatcar Container Linux](https://www.flatcar.org/blog/) | Immutable Linux hosts, container infrastructure, updates and operating-system engineering. | [RSS/Atom](https://www.flatcar.org/blog/index.xml) |

### Kernel development and container internals

| Source | Reading focus | Feed |
| --- | --- | --- |
| [Bootlin](https://bootlin.com/) | Linux drivers, kernel subsystems, embedded systems and upstream engineering. | [RSS/Atom](https://bootlin.com/feed/) |
| [Kernel developer blogs](https://people.kernel.org/read) | First-hand Linux kernel development, maintainer perspectives and systems internals. | [RSS/Atom](https://people.kernel.org/read/feed/) |
| [iximiuz](https://labs.iximiuz.com/) | Linux namespaces, containers, runtimes, networking and practical debugging. | [RSS/Atom](https://iximiuz.com/feed.atom) |

### Programmable networks and Internet engineering

| Source | Reading focus | Feed |
| --- | --- | --- |
| [eBPF.io](https://ebpf.io/blog/) | eBPF development, kernel tracing, programmable networking and production case studies. | [RSS/Atom](https://ebpf.io/blog/rss.xml) |
| [APNIC Blog](https://blog.apnic.net/) | BGP, IPv6, DNS, routing security, measurements and Internet operations. | [RSS/Atom](https://blog.apnic.net/feed/) |
| [ipSpace.net](https://blog.ipspace.net/) | Routing, overlays, data-center networks, automation and protocol design. | [RSS/Atom](https://blog.ipspace.net/atom.xml) |

## Complementary reading without automatic subscriptions

- [Amazon Builders’ Library](https://aws.amazon.com/builders-library/):
  resilience and distributed systems. No current general feed was established
  for this selection; AWS Architecture is a different editorial source.
- [ACM Queue](https://queue.acm.org/): systems engineering and performance.
  The researched feed endpoint returned 403, so it was not bundled.
- [Papers We Love](https://paperswelove.org/): papers and community meetings;
  no public article RSS was established for this selection.
- [daily.dev](https://daily.dev/): personalized discovery through its own
  service; no public RSS API for the user's account was assumed.
- [CNI specification and plugins](https://www.cni.dev/): the interface contract
  behind pod networking. No article feed was established for this site.
- [Antrea](https://antrea.io/) and [Calico](https://www.tigera.io/blog/): compare
  alternative CNI/dataplane designs. The researched feed endpoints could not
  be qualified, so these stay manual references rather than guessed feeds.
- [Google SRE](https://sre.google/resources/): open books and resources.
- The Morning Paper is included as an archive, with no promise of regular
  new publications.

## Articles to start with

- [Timeouts, retries and backoff with jitter](https://aws.amazon.com/builders-library/timeouts-retries-and-backoff-with-jitter/):
  deadlines, retries and load distribution during failures.
- [Saga](https://microservices.io/patterns/data/saga.html): distributed business
  transactions and compensating operations.
- [Transaction Isolation](https://www.postgresql.org/docs/current/transaction-iso.html):
  PostgreSQL guarantees, anomalies and isolation levels.
- [Circuit Breaker](https://martinfowler.com/bliki/CircuitBreaker.html):
  stopping calls when a dependency fails repeatedly.
- [Monitoring Distributed Systems](https://sre.google/sre-book/monitoring-distributed-systems/):
  signals, alerts, symptoms and causes.
- [The Illustrated Transformer](https://jalammar.github.io/illustrated-transformer/):
  a visual introduction to transformer components and attention.

These links are manual reading suggestions. They are not copied into the
cache or reconstructed as publisher articles. Lume stores the content
actually distributed by feeds.
