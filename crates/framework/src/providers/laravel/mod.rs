//! Laravel, as much of it as indexing needs.
//!
//! Deliberately shallow. Laravel's interesting surface is magic -- facades, `__callStatic`, Eloquent
//! attributes, a container that returns what it was told to build -- and none of that is here. What
//! is here is the one thing a *walk* of a Laravel project gets wrong without being told: the
//! framework generates PHP into two directories, and a walk for `*.php` finds it.
//!
//! Both are usually gitignored and discovery honours that, which is exactly why they are named
//! here: it is what makes the answer not depend on whether this checkout ignores them. The same
//! reasoning `sites::excluded_paths` records for Drupal's twig cache, and the same measurement
//! behind it -- eleven thousand generated files stayed out of one index because of one line of a
//! `.gitignore`.
//!
//! Read off each framework's own source rather than off a skeleton's layout, so the paths are what
//! the framework says they are:
//!
//! - `bootstrap/cache/` -- `Application::normalizeCachePath` resolves `cache/services.php`,
//!   `cache/packages.php` and `cache/config.php` through `bootstrapPath()`. Every one is a
//!   `<?php return [...];` blob with no declaration, the shape the generic walk already prunes for
//!   PHPStan and PHPUnit.
//! - `storage/framework/` -- `config/view.php` compiles Blade into `storage_path('framework/views')`,
//!   one `.php` class per template with a hashed name. `cache/`, `sessions/` and `testing/` sit
//!   beside it and hold no code either.
//!
//! What is *not* handled and is worth knowing: a Blade template is named `*.blade.php`, so a walk by
//! extension indexes the template as well as its compiled output. 126 of them ship in the framework
//! itself. They cost a file each and report nothing -- a template with no `<?php` is one long run of
//! inline HTML, which is valid PHP declaring nothing -- and keeping them out needs a seam that can
//! refuse an extension, where this one can only add.

mod aliases;
mod auth;
mod blade;
mod config_text;
mod console;
mod container;
mod factories;
mod schema;
mod translations;
mod views;

use std::path::{Path, PathBuf};

use crate::FrameworkProject;

#[derive(Debug)]
pub struct Laravel;

impl crate::FrameworkProvider for Laravel {
    fn id(&self) -> &'static str {
        "laravel"
    }

    /// An application, or a checkout of the framework itself.
    ///
    /// The application half is both markers and neither alone. `artisan` is a bare script name a
    /// project could plausibly have for its own reasons, and `bootstrap/app.php` is a path a
    /// project could plausibly use without Laravel. Together they are the console entry point and
    /// the application factory that every Laravel app has had for as long as the directory layout
    /// has existed.
    ///
    /// The framework half is new, and the reason it was not here is worth keeping: while this
    /// provider contributed nothing but generated-directory exclusions, "app, not library" was the
    /// right rule -- a library checkout has no `storage/` to keep out. A **contract** is the
    /// opposite kind of fact. It names a framework's own classes, so the checkout that contains
    /// those classes is exactly where it applies, and `laravel/framework`'s 3 055 files of user
    /// code were the one place a Laravel contract could be measured at all: a modern application
    /// hints `Blueprint $table` and writes 0 unhinted callbacks, so an app corpus proves nothing
    /// either way.
    ///
    /// `src/Illuminate/Foundation/Application.php` is that checkout and only that. In an
    /// application the same file sits under `vendor/laravel/framework/`, so the probe cannot
    /// confuse the two, and a project vendoring `illuminate/database` without the foundation is
    /// correctly not Laravel -- see [`InvokedCallback`](crate::InvokedCallback) for why the
    /// contracts are inert there rather than wrong.
    fn detect(&self, root: &Path) -> bool {
        (root.join("artisan").is_file() && root.join("bootstrap/app.php").is_file())
            || root
                .join("src/Illuminate/Foundation/Application.php")
                .is_file()
    }

    /// What Laravel's own source hands the callbacks it is given.
    ///
    /// Every entry was read off the call in the framework that invokes it, because the position is
    /// the whole of what can go wrong silently: naming the wrong one types a parameter that
    /// receives something else, and nothing fails. The two container families invoke from opposite
    /// ends -- `Container::build` calls `$concrete($this, $parameters)` and `extend` calls
    /// `$closure($this->instances[$abstract], $this)` -- so one has the container first and the
    /// other second.
    ///
    /// `Schema\Builder::create` and `table` pass their callback to `Blueprint::__construct`, which
    /// calls `$callback($this)` and documents it as `(\Closure(self): void)|null`. That docblock is
    /// a signature phrost reads, and it is on the *constructor* rather than on the method a
    /// migration calls -- which is why the fact still has to be stated here.
    ///
    /// The resolving family appears twice each because `resolving($abstract, $callback)` is also
    /// written `resolving($callback)`; the two are the same claim about different argument
    /// positions.
    /// Which methods read a container alias, and what its keys name.
    ///
    /// Three method names for one fact -- `$app['config']` is `offsetGet('config')`, whose body is
    /// `return $this->make($offset);`, and the contract's `get` forwards to the same place. See
    /// `container` for why the catalog is open and why the list is static.
    fn argument_bindings(&self) -> &'static [crate::ArgumentBinding] {
        const CONTAINER: &str = "Illuminate\\Contracts\\Container\\Container";

        &[
            crate::ArgumentBinding {
                class: CONTAINER,
                method: "make",
                at: 0,
                suffix: "",
                catalog: container::CATALOG,
            },
            // The subscript spelling, which is seven times commoner than `make` in the framework's
            // own source. Two classes and neither is redundant.
            //
            // The concrete container, which is where `offsetGet` actually lives -- it comes from
            // `ArrayAccess`, and neither container contract declares it.
            crate::ArgumentBinding {
                class: "Illuminate\\Container\\Container",
                method: "offsetGet",
                at: 0,
                suffix: "",
                catalog: container::CATALOG,
            },
            // And the *contract*, which is what a receiver is declared as:
            // `ServiceProvider::$app` is a `@var \Illuminate\Contracts\Foundation\Application`,
            // and an interface does not descend from the class implementing it. Without this entry
            // every one of those 575 sites answers nothing, which is how the first version of this
            // was measured.
            //
            // Stating it is a judgement rather than a reading: the interface does not declare
            // `offsetGet`, so `$this->app['config']` is not valid by its declared type -- Laravel
            // relies on the object always being the concrete `Application`, which it is. The same
            // call this project already made for what a container callback receives, and safe in
            // the same direction: a catalog binding only ever *adds* a resolution for a literal
            // that is a key, and the catalog is open, so it can report nothing.
            crate::ArgumentBinding {
                class: "Illuminate\\Contracts\\Foundation\\Application",
                method: "offsetGet",
                at: 0,
                suffix: "",
                catalog: container::CATALOG,
            },
            // PSR-11, which Laravel's container implements by forwarding to `make`.
            crate::ArgumentBinding {
                class: CONTAINER,
                method: "get",
                at: 0,
                suffix: "",
                catalog: container::CATALOG,
            },
        ]
    }

    /// The helpers that reach the same container without a receiver: `app('router')` is
    /// `Container::getInstance()->make('router')`, and `resolve` is `app` by another name.
    fn function_bindings(&self) -> &'static [crate::FunctionBinding] {
        &[
            crate::FunctionBinding {
                function: "app",
                at: 0,
                catalog: container::CATALOG,
            },
            crate::FunctionBinding {
                function: "resolve",
                at: 0,
                catalog: container::CATALOG,
            },
            // The template's name, which goes to its file and is offered from the ones that exist.
            crate::FunctionBinding {
                function: "view",
                at: 0,
                catalog: views::CATALOG,
            },
            // And an include's, which a template reader writes as a call of its own.
            crate::FunctionBinding {
                function: blade::INCLUDE,
                at: 0,
                catalog: views::CATALOG,
            },
            crate::FunctionBinding {
                function: "trans",
                at: 0,
                catalog: translations::CATALOG,
            },
            crate::FunctionBinding {
                function: "__",
                at: 0,
                catalog: translations::CATALOG,
            },
        ]
    }

    /// Blade, read as the PHP its expressions are.
    fn template_dialects(&self) -> &'static [crate::TemplateDialect] {
        &[crate::TemplateDialect {
            suffix: views::SUFFIX,
            to_php: blade::to_php,
            directory: views::DIRECTORY,
            separator: views::SEPARATOR,
            // `ShareErrorsFromSession`, in the web middleware group of every application.
            shared: &[("errors", "Illuminate\\Support\\ViewErrorBag")],
            components: &[
                crate::ComponentTag {
                    function: blade::COMPONENT,
                    prefix: "components.",
                    shared: &[
                        ("attributes", "Illuminate\\View\\ComponentAttributeBag"),
                        ("slot", "Illuminate\\View\\ComponentSlot"),
                    ],
                },
                // `<livewire:likes :subject="$post" />`: the component's template, handed what its
                // tag writes -- which is what `mount()` or the public property of that name takes.
                crate::ComponentTag {
                    function: blade::LIVEWIRE,
                    prefix: "livewire.",
                    shared: &[],
                },
            ],
            includes: Some(blade::INCLUDE),
            problems: blade::problems,
        }]
    }

    /// `View::share('dates', ...)`, which a service provider calls to hand every view a variable.
    fn shared_template_data(&self) -> &'static [crate::SharedTemplateData] {
        &[crate::SharedTemplateData {
            class: "Illuminate\\Support\\Facades\\View",
            method: "share",
            helper: Some("view"),
        }]
    }

    /// `view('posts.show', [...])`, which is how a controller hands a template its variables.
    fn rendered_templates(&self) -> &'static [crate::RenderedTemplate] {
        &[crate::RenderedTemplate {
            function: "view",
            name_at: 0,
            data_at: 1,
        }]
    }

    /// A Livewire component, which is the `$this` of the template it renders and hands it every
    /// public property it declares.
    fn rendering_classes(&self) -> &'static [crate::RenderingClass] {
        &[crate::RenderingClass {
            class: "Livewire\\Component",
            namespace: "App\\Livewire",
            templates: "livewire",
        }]
    }

    /// A translation key names a line unless the language files say it names an array.
    fn unlisted_keys(&self) -> &'static [crate::UnlistedKey] {
        &[crate::UnlistedKey {
            catalog: translations::CATALOG,
            holds: phrost_model::BuiltinTy::String,
            outside: "::",
        }]
    }

    /// A model's local scope, `scopePublished($query)`, called as `published()` on the model and on
    /// a builder for it. `Model::__call` hands it to a fresh builder, and the builder's own `__call`
    /// finds the method on its model, passes itself in and hands itself back.
    fn prefixed_methods(&self) -> &'static [crate::PrefixedMethod] {
        &[crate::PrefixedMethod {
            class: "Illuminate\\Database\\Eloquent\\Model",
            prefix: "scope",
            builder: "Illuminate\\Database\\Eloquent\\Builder",
            marker: Some("Illuminate\\Database\\Eloquent\\Attributes\\Scope"),
        }]
    }

    /// Dynamic wheres: `Post::whereEmail($email)` is `where('email', '=', $email)` on the builder.
    fn fluent_prefixes(&self) -> &'static [crate::FluentPrefix] {
        &[crate::FluentPrefix {
            class: "Illuminate\\Database\\Query\\Builder",
            prefix: "where",
        }]
    }

    /// A model's relation read as a property: `$post->author` is `author()->getResults()`, which
    /// is what `Model::getRelationValue` runs once `__get` finds a method returning a `Relation`.
    fn method_backed_properties(&self) -> &'static [crate::MethodBackedProperty] {
        &[crate::MethodBackedProperty {
            class: "Illuminate\\Database\\Eloquent\\Model",
            returns: "Illuminate\\Database\\Eloquent\\Relations\\Relation",
            through: "getResults",
            counted: Some("_count"),
            // `getForeignKeyName()` defaults to the relation's name in snake case and the related
            // key: `account()` reads `account_id`. A key named otherwise finds no such column and
            // stays nullable, which is the direction to be wrong in.
            keyed: &[crate::KeyedRelation {
                relation: "Illuminate\\Database\\Eloquent\\Relations\\BelongsTo",
                suffix: "_id",
            }],
        }]
    }

    /// `Collection::macro('sortByCollator', fn ...)`: a method given to any class using `Macroable`.
    fn macro_registrars(&self) -> &'static [crate::MacroRegistrar] {
        &[crate::MacroRegistrar {
            provider: "Illuminate\\Support\\Traits\\Macroable",
            method: "macro",
        }]
    }

    /// The pages a query builds, which releases before 12.x declare bare: `LengthAwarePaginator` where
    /// later ones write `LengthAwarePaginator<int, TModel>`. A relation reaches the builder's through
    /// its `@mixin`; `BelongsToMany` and `HasManyThrough` declare their own, of the related model.
    fn filled_returns(&self) -> &'static [crate::FilledReturn] {
        use crate::FilledArgument::{Builtin, Template};
        const OF_MODEL: &[crate::FilledArgument] =
            &[Builtin(phrost_model::BuiltinTy::Int), Template("TModel")];
        const OF_RELATED: &[crate::FilledArgument] = &[
            Builtin(phrost_model::BuiltinTy::Int),
            Template("TRelatedModel"),
        ];
        const fn page(
            class: &'static str,
            method: &'static str,
            arguments: &'static [crate::FilledArgument],
        ) -> crate::FilledReturn {
            crate::FilledReturn {
                class,
                method,
                arguments,
            }
        }
        const BUILDER: &str = "Illuminate\\Database\\Eloquent\\Builder";
        const BELONGS_TO_MANY: &str = "Illuminate\\Database\\Eloquent\\Relations\\BelongsToMany";
        const THROUGH: &str = "Illuminate\\Database\\Eloquent\\Relations\\HasManyThrough";
        const PAGES: &[crate::FilledReturn] = &[
            page(BUILDER, "paginate", OF_MODEL),
            page(BUILDER, "simplePaginate", OF_MODEL),
            page(BUILDER, "cursorPaginate", OF_MODEL),
            page(BELONGS_TO_MANY, "paginate", OF_RELATED),
            page(BELONGS_TO_MANY, "simplePaginate", OF_RELATED),
            page(BELONGS_TO_MANY, "cursorPaginate", OF_RELATED),
            page(THROUGH, "paginate", OF_RELATED),
            page(THROUGH, "simplePaginate", OF_RELATED),
            page(THROUGH, "cursorPaginate", OF_RELATED),
        ];
        PAGES
    }

    /// The signed-in user, as `config/auth.php` names its model -- see `auth.rs`, which files it only
    /// where every provider names the same one, so it is the answer whichever guard is asked.
    fn keyed_returns(&self) -> &'static [crate::KeyedReturn] {
        const USER: crate::KeyedReturn = crate::KeyedReturn {
            class: "",
            method: "user",
            catalog: auth::CATALOG,
            key: auth::KEY,
            nullable: true,
        };
        &[
            crate::KeyedReturn {
                class: "Illuminate\\Http\\Request",
                ..USER
            },
            crate::KeyedReturn {
                class: "Illuminate\\Contracts\\Auth\\Guard",
                ..USER
            },
            // `auth()->user()`: the factory forwards to the default guard -- see the implied mixin.
            crate::KeyedReturn {
                class: "Illuminate\\Contracts\\Auth\\Factory",
                ..USER
            },
            // `Auth::user()`, which the facade declares as the contract.
            crate::KeyedReturn {
                class: "Illuminate\\Support\\Facades\\Auth",
                ..USER
            },
        ]
    }

    /// A model factory makes one model unless the chain counted: `count(3)`, `times(3)`, or a count
    /// handed to `factory(3)` or `Factory::new(3)`.
    fn chained_operands(&self) -> &'static [crate::ChainedOperand] {
        &[crate::ChainedOperand {
            class: "Illuminate\\Database\\Eloquent\\Factories\\Factory",
            methods: &["create", "make", "createQuietly"],
            after: &["count", "times", "forEachSequence"],
            counted_by: &["factory", "new"],
            many: "Illuminate\\Database\\Eloquent\\Collection",
        }]
    }

    /// A model's accessors read as properties, in both spellings `Model::getAttribute` finds:
    /// `getExcerptAttribute()` and `excerpt(): Attribute` are each `$post->excerpt`.
    fn accessor_properties(&self) -> &'static [crate::AccessorProperty] {
        &[crate::AccessorProperty {
            class: "Illuminate\\Database\\Eloquent\\Model",
            prefix: "get",
            suffix: "Attribute",
            attribute: "Illuminate\\Database\\Eloquent\\Casts\\Attribute",
        }]
    }

    /// A console command's arguments and options, as its `$signature` declares them -- see
    /// `console.rs` for the grammar and what each spelling holds.
    fn declared_inputs(&self) -> &'static [crate::DeclaredInputs] {
        &[crate::DeclaredInputs {
            class: "Illuminate\\Console\\Command",
            property: "signature",
            readers: &[("argument", ""), ("option", "--")],
            parse: console::inputs,
            always: console::APPLICATION_INPUTS,
            unless: &["configure", "addOption", "addArgument"],
        }]
    }

    /// `Contact::factory()`, which is the factory named after the model -- see `factories.rs`.
    fn named_after_caller(&self) -> &'static [crate::NamedAfterCaller] {
        &[crate::NamedAfterCaller {
            class: "Illuminate\\Database\\Eloquent\\Factories\\HasFactory",
            method: "factory",
            unless: &["newFactory", "factory"],
            app_directory: "app",
            default_namespace: "App\\",
            name: factories::factory_for,
        }]
    }

    /// A factory's model, where it writes neither `@extends Factory<User>` nor `$model` -- see
    /// `factories.rs`.
    fn named_template_arguments(&self) -> &'static [crate::NamedTemplateArgument] {
        &[crate::NamedTemplateArgument {
            class: "Illuminate\\Database\\Eloquent\\Factories\\Factory",
            template: "TModel",
            from_property: Some("model"),
            unless: &["model"],
            app_directory: "app",
            default_namespace: "App\\",
            names: factories::model_for,
        }]
    }

    /// A model's columns, as its table's migrations write them -- see `schema.rs` for what each
    /// builder call adds and which table a model's rows are in.
    fn written_schema(&self) -> &'static [crate::WrittenSchema] {
        &[crate::WrittenSchema {
            facade: "Illuminate\\Support\\Facades\\Schema",
            create: &["create"],
            alter: &["table"],
            drop: &["drop", "dropIfExists"],
            rename: &["rename"],
            runs: "up",
            directory: "migrations",
            columns: schema::columns,
            model: "Illuminate\\Database\\Eloquent\\Model",
            table_property: "table",
            default_table: schema::default_table,
        }]
    }

    /// A model's attributes, as far as the model says: what `$casts` or `casts()` turns each into,
    /// and that every name `$fillable` lists is one. The vocabulary is `HasAttributes::castAttribute`
    /// as Laravel 12 writes it.
    ///
    /// Nullable, every one, unless the column says otherwise: a cast of a null column is a null --
    /// `castAttribute` returns it before any cast runs -- and whether a column can be null is in a
    /// migration, read through `written_schema`. Where no migration writes the column, the strict
    /// answer is the one the null-safety rule asks for.
    fn stated_properties(&self) -> &'static [crate::StatedProperties] {
        use crate::StatedType::{Builtin, Class};
        use phrost_model::BuiltinTy::{Array, Bool, Float, Int, String};
        const CARBON: &str = "Illuminate\\Support\\Carbon";
        const IMMUTABLE: &str = "Carbon\\CarbonImmutable";
        const COLLECTION: &str = "Illuminate\\Support\\Collection";
        &[crate::StatedProperties {
            class: "Illuminate\\Database\\Eloquent\\Model",
            typed: &["casts"],
            listed: &["fillable"],
            listed_by_attribute: &["Illuminate\\Database\\Eloquent\\Attributes\\Fillable"],
            spellings: &[
                ("int", Builtin(Int)),
                ("integer", Builtin(Int)),
                ("real", Builtin(Float)),
                ("float", Builtin(Float)),
                ("double", Builtin(Float)),
                ("decimal", Builtin(String)),
                ("string", Builtin(String)),
                ("bool", Builtin(Bool)),
                ("boolean", Builtin(Bool)),
                ("object", Class("stdClass")),
                ("array", Builtin(Array)),
                ("json", Builtin(Array)),
                ("json:unicode", Builtin(Array)),
                ("collection", Class(COLLECTION)),
                ("date", Class(CARBON)),
                ("datetime", Class(CARBON)),
                ("custom_datetime", Class(CARBON)),
                ("immutable_date", Class(IMMUTABLE)),
                ("immutable_datetime", Class(IMMUTABLE)),
                ("immutable_custom_datetime", Class(IMMUTABLE)),
                ("timestamp", Builtin(Int)),
                ("hashed", Builtin(String)),
            ],
            wrapping: &["encrypted:"],
            classes: &[
                (
                    "Illuminate\\Database\\Eloquent\\Casts\\AsCollection",
                    Class(COLLECTION),
                ),
                (
                    "Illuminate\\Database\\Eloquent\\Casts\\AsArrayObject",
                    Class("Illuminate\\Database\\Eloquent\\Casts\\ArrayObject"),
                ),
                (
                    "Illuminate\\Database\\Eloquent\\Casts\\AsStringable",
                    Class("Illuminate\\Support\\Stringable"),
                ),
            ],
            custom: Some((
                "Illuminate\\Contracts\\Database\\Eloquent\\CastsAttributes",
                "get",
            )),
            implicit: &[("created_at", Class(CARBON)), ("updated_at", Class(CARBON))],
            // `getCasts()` adds `[getKeyName() => getKeyType()]` while `getIncrementing()`, and a
            // model not yet saved has no key -- so `?int`, as strict as every cast here.
            key: Some(crate::StatedKey {
                name: "id",
                ty: Builtin(Int),
                unless: &[
                    "primaryKey",
                    "keyType",
                    "incrementing",
                    "getKeyName",
                    "getKeyType",
                    "getIncrementing",
                ],
            }),
            nullable: true,
        }]
    }

    /// Eloquent's model forwards what it does not declare to a query builder for itself:
    /// `__callStatic` makes a `new static` and `__call` hands the call to `newQuery()`. So
    /// `User::where(...)->first()` is a `Builder<User>` call returning a `User`, and nothing but a
    /// method body says so.
    ///
    /// And the authentication factory, which is what `auth()` and `Auth::` hand back: the contract
    /// declares `guard()` and nothing else, and the `AuthManager` the container always binds to it
    /// forwards everything else to the default guard -- its own docblock says `@mixin
    /// StatefulGuard`. So `auth()->user()`, the commonest line in a controller, was an undefined
    /// method on every project. A judgement about the binding, the same one the container's
    /// `offsetGet` entries make about `Application` -- and made about the manager itself rather than
    /// the guard contract, since `auth()->getDefaultDriver()` is the manager's own.
    fn implied_mixins(&self) -> &'static [crate::ImpliedMixin] {
        &[
            crate::ImpliedMixin {
                class: "Illuminate\\Database\\Eloquent\\Model",
                mixin: "Illuminate\\Database\\Eloquent\\Builder",
                with_static: true,
            },
            crate::ImpliedMixin {
                class: "Illuminate\\Contracts\\Auth\\Factory",
                mixin: "Illuminate\\Auth\\AuthManager",
                with_static: false,
            },
            // The same judgement for the cache and the application: `Cache::store()` and `cache()`
            // declare the contract, and the container binds it to the concrete repository, which
            // declares `tags()` and hands what it does not declare -- `lock()`, `flexible()` -- to
            // the store through `__call`. 132 `lock()` and 78 `tags()` calls in Laravel's own source
            // and tests were undefined on the interface, and every application writes them.
            crate::ImpliedMixin {
                class: "Illuminate\\Contracts\\Cache\\Repository",
                mixin: "Illuminate\\Cache\\Repository",
                with_static: false,
            },
            crate::ImpliedMixin {
                class: "Illuminate\\Contracts\\Foundation\\Application",
                mixin: "Illuminate\\Foundation\\Application",
                with_static: false,
            },
            // `Storage::disk()` and `DB::connection()` likewise: the filesystem contract is always
            // the adapter (`url()`, `temporaryUrl()`, the testing assertions), and a connection is
            // always one of the framework's `Connection` classes (`getSchemaBuilder()`).
            crate::ImpliedMixin {
                class: "Illuminate\\Contracts\\Filesystem\\Filesystem",
                mixin: "Illuminate\\Filesystem\\FilesystemAdapter",
                with_static: false,
            },
            crate::ImpliedMixin {
                class: "Illuminate\\Database\\ConnectionInterface",
                mixin: "Illuminate\\Database\\Connection",
                with_static: false,
            },
            // `trans()` with no key and `Mail::mailer()` declare the contracts, and the container
            // has one class for each: `has()` and `getSymfonyTransport()` are the concrete's.
            crate::ImpliedMixin {
                class: "Illuminate\\Contracts\\Translation\\Translator",
                mixin: "Illuminate\\Translation\\Translator",
                with_static: false,
            },
            crate::ImpliedMixin {
                class: "Illuminate\\Contracts\\Mail\\Mailer",
                mixin: "Illuminate\\Mail\\Mailer",
                with_static: false,
            },
        ]
    }

    fn mixins_returned_by(&self) -> &'static [crate::MixinReturnedBy] {
        &[crate::MixinReturnedBy {
            method: "getFacadeAccessor",
        }]
    }

    /// The facades whose generated tags dropped a template their root declares: the cache's
    /// closure-taking methods, the container's `make()`, the connection's `transaction()`.
    fn methods_tagged_from(&self) -> &'static [crate::MethodsTaggedFrom] {
        &[
            crate::MethodsTaggedFrom {
                class: "Illuminate\\Support\\Facades\\Cache",
                source: "Illuminate\\Cache\\Repository",
            },
            crate::MethodsTaggedFrom {
                class: "Illuminate\\Support\\Facades\\App",
                source: "Illuminate\\Foundation\\Application",
            },
            crate::MethodsTaggedFrom {
                class: "Illuminate\\Support\\Facades\\DB",
                source: "Illuminate\\Database\\Connection",
            },
        ]
    }

    fn opened_by_call(&self) -> &'static [crate::OpenedByCall] {
        &[crate::OpenedByCall {
            class: "Illuminate\\Database\\Eloquent\\Model",
            methods: &[
                "increment",
                "decrement",
                "incrementQuietly",
                "decrementQuietly",
            ],
        }]
    }

    fn invoked_callbacks(&self) -> &'static [crate::InvokedCallback] {
        const APPLICATION: Option<&str> = Some("Illuminate\\Foundation\\Application");
        // The *contract*, not the concrete class, because that is what a receiver is declared as:
        // `ServiceProvider::$app` is a `@var \Illuminate\Contracts\Foundation\Application`, and
        // an interface does not descend from the class that implements it -- so an entry naming
        // `Illuminate\Container\Container` matched nothing in any service provider, which is every
        // site the entries exist for. The concrete class implements this, so naming it here covers
        // both directions. Measured wrong first: the schema entries fired and the container ones
        // resolved nothing, on a corpus where the coverage number still moved by 562.
        const CONTAINER: &str = "Illuminate\\Contracts\\Container\\Container";
        // `rebinding` is the one of these the contract does not declare, so it is named on the
        // class that does.
        const CONCRETE: &str = "Illuminate\\Container\\Container";
        const SCHEMA: &str = "Illuminate\\Database\\Schema\\Builder";
        const QUERY: &str = "Illuminate\\Database\\Query\\Builder";
        const JOIN: &str = "Illuminate\\Database\\Query\\JoinClause";

        &[
            // A nested condition: `->where(function ($query) { ... })` hands the closure a fresh
            // builder from `forNestedWhere()`, and `whereExists` one from `forSubQuery()`. The query
            // builder documents the parameter as a bare `\Closure`, so nothing else says what the
            // closure receives -- 99 `$query->where` on the framework's own corpus read untyped.
            crate::InvokedCallback {
                class: QUERY,
                method: "where",
                argument: 0,
                parameters: &[Some(QUERY)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "orWhere",
                argument: 0,
                parameters: &[Some(QUERY)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "whereNot",
                argument: 0,
                parameters: &[Some(QUERY)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "orWhereNot",
                argument: 0,
                parameters: &[Some(QUERY)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "whereNested",
                argument: 0,
                parameters: &[Some(QUERY)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "havingNested",
                argument: 0,
                parameters: &[Some(QUERY)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "whereExists",
                argument: 0,
                parameters: &[Some(QUERY)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "orWhereExists",
                argument: 0,
                parameters: &[Some(QUERY)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "whereNotExists",
                argument: 0,
                parameters: &[Some(QUERY)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "orWhereNotExists",
                argument: 0,
                parameters: &[Some(QUERY)],
            },
            // A join's condition: `$first($join)` with the `JoinClause` being built, which is the
            // third argument of the `...Sub` spellings because the subquery and its alias come first.
            crate::InvokedCallback {
                class: QUERY,
                method: "join",
                argument: 1,
                parameters: &[Some(JOIN)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "leftJoin",
                argument: 1,
                parameters: &[Some(JOIN)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "rightJoin",
                argument: 1,
                parameters: &[Some(JOIN)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "joinSub",
                argument: 2,
                parameters: &[Some(JOIN)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "leftJoinSub",
                argument: 2,
                parameters: &[Some(JOIN)],
            },
            crate::InvokedCallback {
                class: QUERY,
                method: "rightJoinSub",
                argument: 2,
                parameters: &[Some(JOIN)],
            },
            // A migration's `Schema::create('t', function ($table) { ... })`. 226 sites in the
            // framework's own source and tests, and every migration a project ever writes.
            crate::InvokedCallback {
                class: SCHEMA,
                method: "create",
                argument: 1,
                parameters: &[Some("Illuminate\\Database\\Schema\\Blueprint")],
            },
            crate::InvokedCallback {
                class: SCHEMA,
                method: "table",
                argument: 1,
                parameters: &[Some("Illuminate\\Database\\Schema\\Blueprint")],
            },
            // A service provider's `$this->app->singleton(Foo::class, function ($app) { ... })`.
            // `$parameters` is second and is an array of overrides, so it is left unnamed rather
            // than given a place.
            crate::InvokedCallback {
                class: CONTAINER,
                method: "bind",
                argument: 1,
                parameters: &[APPLICATION],
            },
            crate::InvokedCallback {
                class: CONTAINER,
                method: "bindIf",
                argument: 1,
                parameters: &[APPLICATION],
            },
            crate::InvokedCallback {
                class: CONTAINER,
                method: "singleton",
                argument: 1,
                parameters: &[APPLICATION],
            },
            crate::InvokedCallback {
                class: CONTAINER,
                method: "singletonIf",
                argument: 1,
                parameters: &[APPLICATION],
            },
            crate::InvokedCallback {
                class: CONTAINER,
                method: "scoped",
                argument: 1,
                parameters: &[APPLICATION],
            },
            // The other end: the thing being decorated comes first and its class is whatever was
            // bound, which nothing here knows. The hole is what keeps the container in position 1.
            crate::InvokedCallback {
                class: CONTAINER,
                method: "extend",
                argument: 1,
                parameters: &[None, APPLICATION],
            },
            // `$callback($this, $instance)` -- container first here, instance second.
            crate::InvokedCallback {
                class: CONCRETE,
                method: "rebinding",
                argument: 1,
                parameters: &[APPLICATION, None],
            },
            // `fireCallbackArray` calls `$callback($object, $this)`.
            crate::InvokedCallback {
                class: CONTAINER,
                method: "resolving",
                argument: 0,
                parameters: &[None, APPLICATION],
            },
            crate::InvokedCallback {
                class: CONTAINER,
                method: "resolving",
                argument: 1,
                parameters: &[None, APPLICATION],
            },
            crate::InvokedCallback {
                class: CONTAINER,
                method: "afterResolving",
                argument: 0,
                parameters: &[None, APPLICATION],
            },
            crate::InvokedCallback {
                class: CONTAINER,
                method: "afterResolving",
                argument: 1,
                parameters: &[None, APPLICATION],
            },
            // `fireBeforeCallbackArray` calls `$callback($abstract, $parameters, $this)` -- a
            // string, an array, then the container.
            crate::InvokedCallback {
                class: CONTAINER,
                method: "beforeResolving",
                argument: 0,
                parameters: &[None, None, APPLICATION],
            },
            crate::InvokedCallback {
                class: CONTAINER,
                method: "beforeResolving",
                argument: 1,
                parameters: &[None, None, APPLICATION],
            },
        ]
    }

    fn project(&self, root: &Path) -> Box<dyn FrameworkProject> {
        Box::new(LaravelProject {
            root: root.to_path_buf(),
        })
    }
}

#[derive(Debug)]
struct LaravelProject {
    root: PathBuf,
}

impl FrameworkProject for LaravelProject {
    fn catalog_places(&self) -> Vec<crate::PlaceEntry> {
        views::places(&self.root)
    }

    fn class_aliases(&self) -> Vec<crate::ClassAlias> {
        aliases::class_aliases(&self.root)
    }

    fn catalog_entries(&self) -> Vec<crate::CatalogEntry> {
        let mut entries = container::catalog_entries();
        entries.extend(auth::user_model(&self.root));
        entries.extend(translations::array_keys(&self.root));
        entries
    }

    /// Only what exists, so a project that has never been booted contributes nothing and a walk
    /// wastes no comparison on a path that is not there.
    fn excluded_paths(&self) -> Vec<PathBuf> {
        ["bootstrap/cache", "storage/framework"]
            .iter()
            .map(|generated| self.root.join(generated))
            .filter(|path| path.is_dir())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FrameworkProvider;

    /// Both markers for an application, and the framework's own source on its own.
    ///
    /// A bare `artisan` script or a lone `bootstrap/app.php` is something another project can have,
    /// and reading either as Laravel would exclude directories on a project that never generated
    /// them. The framework's checkout has neither and is detected by its own source instead, which
    /// is what lets a Laravel contract reach the only corpus that writes the shapes it is about.
    #[test]
    fn an_application_and_the_framework_are_both_detected() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        assert!(!Laravel.detect(root), "an empty directory is not Laravel");

        std::fs::write(root.join("artisan"), "#!/usr/bin/env php\n").expect("artisan");
        assert!(
            !Laravel.detect(root),
            "the console script alone says nothing"
        );

        std::fs::create_dir_all(root.join("bootstrap")).expect("bootstrap");
        std::fs::write(root.join("bootstrap/app.php"), "<?php\n").expect("app.php");
        assert!(
            Laravel.detect(root),
            "both markers together are an application"
        );

        // The framework's own checkout, which has neither marker. Its source at the *root* is what
        // says so: in an application the same path sits under `vendor/laravel/framework/`, so a
        // project vendoring Laravel is not mistaken for Laravel's repository -- and one vendoring
        // `illuminate/container` alone has no foundation to find.
        let library = tempfile::tempdir().expect("a temporary directory");
        let library = library.path();
        std::fs::create_dir_all(library.join("src/Illuminate/Container")).expect("container");
        std::fs::write(
            library.join("src/Illuminate/Container/Container.php"),
            "<?php\n",
        )
        .expect("Container.php");
        assert!(
            !Laravel.detect(library),
            "the container package alone is not Laravel"
        );

        std::fs::create_dir_all(library.join("src/Illuminate/Foundation")).expect("foundation");
        std::fs::write(
            library.join("src/Illuminate/Foundation/Application.php"),
            "<?php\n",
        )
        .expect("Application.php");
        assert!(
            Laravel.detect(library),
            "the framework's own source is Laravel"
        );
    }

    /// Every entry names a parameter position the framework really passes there.
    ///
    /// Not a check that the classes exist -- `every_contract_names_a_class_the_index_could_hold`
    /// does that for every kind at once. This pins the two things only this list knows: that the
    /// container families invoke from opposite ends, and that a method taking its callback in two
    /// parameters has an entry for each. Both are facts a reader can only get wrong silently, since
    /// a wrong position types a parameter that receives something else.
    #[test]
    fn the_container_families_pass_from_opposite_ends() {
        let entries = Laravel.invoked_callbacks();
        let of = |method: &str, argument: usize| {
            entries
                .iter()
                .find(|entry| entry.method == method && entry.argument == argument)
                .map(|entry| entry.parameters)
        };

        // `Container::build` calls `$concrete($this, $parameters)`: the container is first.
        assert_eq!(
            of("singleton", 1),
            Some(&[Some(r"Illuminate\Foundation\Application")][..])
        );
        // `extend` calls `$closure($this->instances[$abstract], $this)`: the service is first and
        // its class is whatever was bound, so the hole is what keeps the container in place.
        assert_eq!(
            of("extend", 1),
            Some(&[None, Some(r"Illuminate\Foundation\Application")][..])
        );
        // `rebinding` fires `$callback($this, $instance)` -- the other way round again.
        assert_eq!(
            of("rebinding", 1),
            Some(&[Some(r"Illuminate\Foundation\Application"), None][..])
        );
        // `resolving($abstract, $callback)` is also written `resolving($callback)`, where the
        // closure fills the *first* parameter. Both, or the one-argument form answers nothing.
        assert!(of("resolving", 0).is_some() && of("resolving", 1).is_some());
        assert!(of("beforeResolving", 0).is_some() && of("beforeResolving", 1).is_some());
        // A migration's callback, which `Blueprint::__construct` calls with `$this`.
        assert_eq!(
            of("create", 1),
            Some(&[Some(r"Illuminate\Database\Schema\Blueprint")][..])
        );
    }

    /// Only the generated directories that exist, and nothing else under them.
    #[test]
    fn the_generated_directories_are_excluded_once_they_exist() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        let project = Laravel.project(root);
        assert!(
            project.excluded_paths().is_empty(),
            "a project that has never been booted generated nothing"
        );

        std::fs::create_dir_all(root.join("bootstrap/cache")).expect("bootstrap/cache");
        std::fs::create_dir_all(root.join("storage/framework/views")).expect("compiled views");
        // Beside them, and code: this must not be swept up with the two.
        std::fs::create_dir_all(root.join("app/Models")).expect("app");

        let excluded = Laravel.project(root).excluded_paths();
        assert_eq!(
            excluded,
            vec![root.join("bootstrap/cache"), root.join("storage/framework")],
            "the two generated directories, in the order they are named"
        );
    }
}
