# Pupoxide: Puppet for the Rust Era

Pupoxide — это высокопроизводительный, надежный и декларативный инструмент управления конфигурациями, вдохновленный Puppet, но переосмысленный для современной эры Rust.

> [!WARNING]
> **Experimental Project / Proof of Concept**
>
> Этот проект является архитектурным экспериментом по переосмыслению идей Puppet на языке Rust. Он **не готов к промышленному использованию**, но активно развивается. Мы рады [новым контрибьюторам](doc/workflow.md)!

---

## 🚀 Почему Pupoxide?

| Особенность | Puppet | Pupoxide |
| :--- | :--- | :--- |
| **Язык** | Ruby (медленный, тяжелый) | **Rust** (максимальная скорость) |
| **Зависимости** | Требует Ruby runtime | **Zero dependencies** (один бинарник) |
| **Безопасность** | Динамический тип | **Static-like safety** + Граф зависимостей (DAG) |
| **DSL** | Кастомный, ограниченный | **Rhai** (мощный, расширяемый скриптинг) |
| **Параллелизм** | Ограниченный | **Нативный параллелизм** независимых ресурсов |

> [!TIP]
> **Главная фишка**: Pupoxide автоматически строит граф зависимостей и выполняет не связанные между собой задачи (например, установку htop и настройку nginx) одновременно, что дает кратный прирост скорости на больших конфигурациях.

---

## 📜 Примеры манифестов (DSL на базе Rhai)

Pupoxide использует мощный и лаконичный синтаксис [Rhai](https://rhai.rs/).

```rust
// Установка пакетов через модуль brew
import "brew" as b;
b::pkg(["htop", "wget"], #{ ensure: "present" });

// Создание директории с правами
directory("/tmp/pupoxide/cache", #{
    ensure: "present",
    mode: "0755",
    owner: "admin"
});

// Файл с содержимым и зависимостью
file("/etc/motd", #{
    ensure: "present",
    content: "Welcome to Pupoxide Node!",
    require: "Directory[/tmp/pupoxide/cache]" // Выполнится ПОСЛЕ создания папки
});

// Условная логика на основе фактов системы
if facts["os_family"] == "Darwin" {
    exec("mac-cleanup", #{
        command: "rm -rf ~/Library/Caches/*",
        only_if: "test -d ~/Library/Caches"
    });
}

// Рендеринг шаблонов Tera для конфигурационных файлов
file("/etc/myapp/config.yaml", #{
    ensure: "present",
    content: std::template("config.tera", #{
        port: 8080,
        enable_ssl: true
    })
});
```

---

## 💻 Использование CLI

### 1. Server-Less режим (Локальное применение)

Применяет конфигурацию сразу на текущей машине. Идеально для деплоя скриптов или локальной настройки.

```bash
# Применить конкретный файл
pupoxide run --file ./site.rhai

# Применить целое окружение (структура Puppet-like)
pupoxide apply --environment production --config ./my_configs

# Только посмотреть изменения (Dry-run)
pupoxide apply --environment production --dry-run
```

### 2. Режим Агент-Сервер (mTLS Security)

Безопасная архитектура с автоматической генерацией сертификатов и трехфазным бутстрапом.

1.  **Запуск Мастера:**
    ```bash
    pupoxide master start --port 8080
    ```
2.  **Запрос на регистрацию (на Агенте):**
    ```bash
    pupoxide agent --server http://master:8080 --node agent-01 --bootstrap
    ```
3.  **Подпись сертификата (на Мастере):**
    ```bash
    pupoxide master sign --node agent-01
    ```
4.  **Регулярная работа (mTLS):**
    ```bash
    pupoxide agent --server https://master:8080 --node agent-01
    ```

---

## 📂 Структура проекта

Pupoxide поощряет паттерн **Roles and Profiles** для чистоты кода.

```text
/etc/pupoxide/
├── environments/
│   └── production/
│       ├── manifests/
│       │   └── site.rhai      # Точка входа (импорт ролей)
│       ├── role/              # Бизнес-логика (например, "web_server.rhai")
│       ├── profile/           # Технологические стеки ("nginx_proxy.rhai")
│       ├── modules/           # Переиспользуемые компоненты (сервисы, пакеты)
│       └── data/              # Иерархические данные (YAML)
└── certs/                 # Хранилище mTLS сертификатов
```

---

## 🛠 Дополнительные инструменты

*   **Визуализация графа**: `pupoxide graph --file site.rhai --style mermaid` — генерирует схему зависимостей.
*   **Сериализация**: `mutex: "id"` — гарантирует, что ресурсы из одной группы будут выполняться строго по очереди, не нарушая общего параллелизма.
*   **Откат (Rollback)**: Каждая транзакция логируется, позволяя вернуть систему в предыдущее состояние.

### 🔒 Мьютекс-группы (Serial Execution)

Некоторые ресурсы (например, менеджеры пакетов) не умеют работать параллельно. Используйте атрибут `mutex` для управления очередью:

```rust
// Эти задачи будут выполнены строго по очереди
pkg("htop", #{ mutex: "brew" });
pkg("wget", #{ mutex: "brew" });

// Эта задача останется независимой и выполнится параллельно
file("/tmp/config", #{ ensure: "present" });
```

## 📖 Документация
- [Техническое видение](doc/vision.md)
- [Соглашения по коду](doc/conventions.md)
- [Архитектурный контекст](doc/architecture_context.md)

---
License: MIT
