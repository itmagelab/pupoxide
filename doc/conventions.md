# Соглашения по разработке

> Правила написания кода для проекта Pupoxide.
> Основано на [техническом видении](vision.md).

## Технологический стек

- **Язык**: Rust (Latest Stable)
- **Async Runtime**: `tokio`
- **Web Server**: `axum` (для Master API)
- **HTTP Client**: `reqwest` (для Agent клиента)
- **TLS/mTLS**: `tokio-rustls` (Agent-Master коммуникация)
- **Certificates**: `rcgen` (генерация и подпись CSR)
- **Database**: PostgreSQL + `sea-orm` + `sea-query`
- **Error Handling**: `anyhow` (везде)
- **Logging/Tracing**: `tracing` + `tracing-subscriber`
- **Serialization**: `serde` + `serde_json` / `serde_yaml`
- **Validation**: `validator`

## Архитектура: Модульный монолит (Hexagonal)

Проект разделен на логические слои внутри `src/`, следуя принципам Ports and Adapters:

1.  **Domain (`src/domain/`)**: Чистая бизнес-логика и Ports (traits). Сущности (entities), правила валидации. Не имеет внешних зависимостей.
2.  **Infrastructure (`src/infrastructure/`)**: Adapters. Реализация I/O (БД, CLI-вызов системных утилит, Файловая система). Зависит от Domain.
3.  **Application (`src/application/`)**: Use Cases. Координация потока данных между Domain и Infrastructure.
4.  **Interface (`src/interface/`)**: CLI (`clap`) и другие точки входа.

## Правила слоев

### Domain Purity (Чистота Домена)
- **ЗАПРЕЩЕНО** зависеть от любых внешних библиотек, кроме `serde`, `thiserror` и `validator`.
- **ЗАПРЕЩЕНО** использовать типы из Infrastructure или Application в Domain.
- Все внешние зависимости (БД, API) описываются через Trait (Ports).

### Data & Serialization
- Используйте `serde` для всех моделей данных, которые должны передаваться между слоями.
- Валидация входных данных должна происходить на уровне Domain через crate `validator`.

### Database (SeaORM)
- Сущности SeaORM (`Entity`) и файлы миграций физически находятся в `src/infrastructure/`.
- Domain не должен знать о SeaORM. Конвертация между моделями БД и доменными моделями происходит в слое Infrastructure.

## Кодовые стандарты

### Rust Idioms
- **ОБЯЗАТЕЛЬНО** использовать `cargo fmt` и `cargo clippy`.
- **ЗАПРЕЩЕНО** использование `unwrap()` и `expect()` в `src/`. Исключение — тесты и явные инварианты (с комментарием).
- **ОБЯЗАТЕЛЬНО** использовать архитектурный линт `#![deny(clippy::unwrap_used)]`.

### Error Handling
- Используйте `anyhow::Result<T>` (т.е. `Result<T, anyhow::Error>`) везде во всем проекте для единообразия.
- Создавайте ошибки с помощью `anyhow::anyhow!()` макроса для контекстных сообщений об ошибках.
- Используйте `.context()` и `.with_context()` для добавления контекста к ошибкам.

### Logging
- Используйте макросы `tracing::info!`, `warn!`, `error!`, `debug!`.
- Всегда добавляйте контекст к логам через поля: `tracing::info!(resource_id = %id, "Processing")`.

## Тестирование
- **TDD (Test-Driven Development)**: Мы используем TDD-подход при разработке. Процесс создания новой логики или исправления багов состоит из трех шагов:
  1. Сначала пишется тест, проверяющий новое поведение или воспроизводящий баг (тест изначально падает).
  2. Пишется минимально необходимый код реализации, чтобы тест стал проходить успешно.
  3. Выполняется рефакторинг кода и тестов при сохранении работоспособности.
- **Unit-тесты** обязательны для логики домена и находятся в тех же файлах.
- **Интеграционные тесты** в `tests/` проверяют цепочку: Interface -> Application -> Infrastructure (с БД).


## Git и Коммиты
- **Язык**: Английский.
- **Формат**: `type: description` (например, `feat: add postgres connection pool`).
- **Workflow**: Спрашивать разрешение перед `push`.

## Коммуникация

- **Язык общения**: Весь проект документируется на русском языке. Общение в чате с ИИ-ассистентом ведется исключительно на русском языке.

## Агент

- **ЗАПРЕЩЕНО** агенту автоматически создавать документы без явного запроса пользователя.

