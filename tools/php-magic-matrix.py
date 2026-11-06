#!/usr/bin/env python3
"""One probe per kind of PHP magic, each in a scratch project of its own.

    python3 tools/php-magic-matrix.py [--binary target/release/phrost] [--only <substring>]

`__get`, `__call`, `__invoke`, `__toString`, `@property`, `@method`, `@mixin`, `ArrayAccess`,
iterators and generators, enums, `static`, class-strings, callable arrays and strings, bound
closures, destructuring, `throw` as an expression: the language's ways of answering a name nothing
declares, or a value nothing typed. Each case is a small PHP file; a marker `‸{want}` stands where a
cursor would, and `want` is text the goto, hover and completion answer at that position must hold
(`a&&b` for both, `!a` for its absence). Every case is also checked, and whatever it reports
beyond an unused variable is printed, so the `fp_` cases are read for silence and the `tp_` ones for
a finding.

One directory per case because two probes declaring one class make every probe lie -- see CLAUDE.md
§8.4. Expectations are written as `code: T $x` wherever a type is meant, because `Foo $x` is also
a substring of `?Foo $x` and a matrix that reads a nullable as a pass measures nothing -- the first
pass scored `$maybe ?? throw ...` as typed while it still held the null, for exactly that reason.

The failures that are left are the roadmap's list, not noise: each one is a gap or a position, and
the roadmap says which.
"""
import argparse
import os
import re
import shutil
import subprocess
import tempfile

H = "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n"

CASES = [
("get_typed_return", H + r'''
class Bag { public function __get(string $k): Foo { return new Foo; } }
function f(Bag $b): void {
    ‸{code: Foo $x}$x = $b->anything;
    $b->anything->‸{id}id;
}'''),
("get_phpdoc_return", H + r'''
class Bag { /** @return Foo */ public function __get($k) { return new Foo; } }
function f(Bag $b): void { ‸{code: Foo $x}$x = $b->anything; }'''),
("property_read_write", H + r'''
/** @property-read Foo $r
 *  @property-write int $w */
class Bag { public function __get($k) {} public function __set($k, $v) {} }
function f(Bag $b): void { ‸{code: Foo $x}$x = $b->r; $b->‸{w}w = 1; $b->r->‸{foo}foo(); }'''),
("property_inherited_from_parent", H + r'''
/** @property Foo $p */
abstract class Base { public function __get($k) {} }
class Child extends Base {}
function f(Child $c): void { ‸{code: Foo $x}$x = $c->p; }'''),
("property_on_interface", H + r'''
/** @property Foo $p */
interface Has {}
function f(Has $c): void { ‸{code: Foo $x}$x = $c->p; }'''),
("property_from_trait", H + r'''
/** @property Foo $p */
trait T {}
class C { use T; public function __get($k) {} }
function f(C $c): void { ‸{code: Foo $x}$x = $c->p; }'''),
("property_completion", H + r'''
/** @property Foo $magicProp */
class Bag { public function __get($k) {} }
function f(Bag $b): void { $b->‸{magicProp}magicProp; }'''),
("call_typed_return", H + r'''
class Bag { public function __call(string $m, array $a): Foo { return new Foo; } }
function f(Bag $b): void { ‸{code: Foo $x}$x = $b->whatever(); }'''),
("call_static_typed", H + r'''
class Bag { public static function __callStatic(string $m, array $a): Foo { return new Foo; } }
function f(): void { ‸{code: Foo $x}$x = Bag::whatever(); }'''),
("method_static_tag", H + r'''
/** @method static Foo make(int $n) */
class Bag { public static function __callStatic($m, $a) {} }
function f(): void { ‸{code: Foo $x}$x = Bag::make(1); Bag::‸{make}make(2); }'''),
("method_static_returns_static", H + r'''
/** @method static static make() */
class Bag { public static function __callStatic($m, $a) {} public function own(): int { return 1; } }
class Sub extends Bag {}
function f(): void { ‸{code: Sub $x}$x = Sub::make(); }'''),
("method_tag_template", H + r'''
/** @method T get<T>(class-string<T> $c) */
class Bag { public function __call($m, $a) {} }
function f(Bag $b): void { ‸{code: Foo $x}$x = $b->get(Foo::class); }'''),
("method_tag_signature_help", H + r'''
/** @method Foo make(int $n, string $s) */
class Bag { public function __call($m, $a) {} }
function f(Bag $b): void { $b->make(1, ‸{code: string $s}'x'); }'''),
("invoke", H + r'''
class Inv { public function __invoke(int $a): Foo { return new Foo; } }
function f(Inv $i): void { ‸{code: Foo $x}$x = $i(1); $i(1)->‸{foo}foo(); }'''),
("invoke_property", H + r'''
class Inv { public function __invoke(): Foo { return new Foo; } }
class Holder { public Inv $inv; }
function f(Holder $h): void { ‸{code: Foo $x}$x = ($h->inv)(); }'''),
("tostring_cast", H + r'''
class S { public function __toString(): string { return ''; } }
function f(S $s): void { ‸{code: string $x}$x = (string) $s; ‸{code: string $y}$y = "{$s}"; ‸{code: string $z}$z = $s . ''; }'''),
("stringable_to_string_param_nonstrict", H + r'''
class S { public function __toString(): string { return ''; } }
function g(string $v): void {}
function f(S $s): void { g($s); }'''),
("arrayaccess_offsetget", H + r'''
class Coll implements \ArrayAccess {
    public function offsetExists(mixed $o): bool { return true; }
    public function offsetGet(mixed $o): Foo { return new Foo; }
    public function offsetSet(mixed $o, mixed $v): void {}
    public function offsetUnset(mixed $o): void {}
}
function f(Coll $c): void { ‸{code: Foo $x}$x = $c['k']; $c['k']->‸{foo}foo(); }'''),
("arrayaccess_template", H + r'''
/** @implements \ArrayAccess<string, Foo> */
class Coll implements \ArrayAccess {
    public function offsetExists(mixed $o): bool { return true; }
    public function offsetGet(mixed $o): mixed { return null; }
    public function offsetSet(mixed $o, mixed $v): void {}
    public function offsetUnset(mixed $o): void {}
}
function f(Coll $c): void { ‸{code: Foo $x}$x = $c['k']; }'''),
("arrayobject_generic", H + r'''
/** @param \ArrayObject<int, Foo> $a */
function f(\ArrayObject $a): void { ‸{code: Foo $x}$x = $a[0]; foreach ($a as ‸{code: Foo $v}$v) {} }'''),
("iteratoraggregate", H + r'''
/** @implements \IteratorAggregate<int, Foo> */
class Coll implements \IteratorAggregate { public function getIterator(): \Iterator { return new \ArrayIterator([]); } }
function f(Coll $c): void { foreach ($c as $k => ‸{code: Foo $v}$v) {} }'''),
("iteratoraggregate_by_return", H + r'''
class Coll implements \IteratorAggregate {
    /** @return \ArrayIterator<int, Foo> */
    public function getIterator(): \ArrayIterator { return new \ArrayIterator([]); }
}
function f(Coll $c): void { foreach ($c as ‸{code: Foo $v}$v) {} }'''),
("iterator_current", H + r'''
class It implements \Iterator {
    public function current(): Foo { return new Foo; }
    public function key(): int { return 0; }
    public function next(): void {}
    public function rewind(): void {}
    public function valid(): bool { return false; }
}
function f(It $c): void { foreach ($c as $k => ‸{code: Foo $v}$v) {} }'''),
("generator_return", H + r'''
/** @return \Generator<int, Foo> */
function gen(): \Generator { yield new Foo; }
function f(): void { foreach (gen() as ‸{code: Foo $v}$v) {} }'''),
("generator_iterable", H + r'''
/** @return iterable<Foo> */
function gen(): iterable { yield new Foo; }
function f(): void { foreach (gen() as ‸{code: Foo $v}$v) {} }'''),
("generator_send", H + r'''
/** @return \Generator<int, Foo, string, int> */
function gen(): \Generator { $s = yield new Foo; return 1; }
function f(): void { $g = gen(); ‸{code: int $r}$r = $g->getReturn(); ‸{code: Foo $c}$c = $g->current(); }'''),
("yield_value_in_body", H + r'''
/** @return \Generator<int, Foo, string, void> */
function gen(): \Generator { ‸{code: string $s}$s = yield new Foo; }'''),
("iterator_to_array", H + r'''
/** @return \Generator<int, Foo> */
function gen(): \Generator { yield new Foo; }
function f(): void { ‸{Foo}$a = iterator_to_array(gen()); }'''),
("mixin", H + r'''
/** @mixin Foo */
class Proxy { public function __call($m, $a) {} }
function f(Proxy $p): void { ‸{code: string $x}$x = $p->foo(); $p->‸{foo}foo(); }'''),
("mixin_template", H + r'''
/**
 * @template T
 * @mixin T
 */
class Proxy { public function __call($m, $a) {} }
/** @param Proxy<Foo> $p */
function f(Proxy $p): void { ‸{code: string $x}$x = $p->foo(); }'''),
("new_static_self", H + r'''
class Base { public static function make(): static { return new static(); } public static function me(): self { return new self(); } }
class Sub extends Base {}
function f(): void { ‸{code: Sub $x}$x = Sub::make(); ‸{code: Base $y}$y = Sub::me(); }'''),
("static_phpdoc_fluent", H + r'''
class Base { /** @return $this */ public function with() { return $this; } }
class Sub extends Base { public function sub(): int { return 1; } }
function f(Sub $s): void { ‸{code: Sub $x}$x = $s->with(); $s->with()->‸{sub}sub(); }'''),
("new_static_in_body", H + r'''
class Base { public function make() { ‸{static}$x = new static(); return $x; } }'''),
("clone", H + r'''
function f(Foo $a): void { ‸{code: Foo $x}$x = clone $a; }'''),
("new_class_string_var", H + r'''
/** @param class-string<Foo> $c */
function f(string $c): void { ‸{code: Foo $x}$x = new $c(); }'''),
("new_string_literal_var", H + r'''
function f(): void { $c = Foo::class; ‸{code: Foo $x}$x = new $c(); }'''),
("static_call_on_class_string", H + r'''
class M { public static function make(): static { return new static; } }
/** @param class-string<M> $c */
function f(string $c): void { ‸{code: M $x}$x = $c::make(); }'''),
("instance_class_const", H + r'''
class M { const X = 'a'; }
function f(M $m): void { ‸{code: string $x}$x = $m::X; ‸{class-string}$y = $m::class; }'''),
("get_class", H + r'''
function f(Foo $m): void { ‸{class-string}$y = get_class($m); ‸{code: Foo $z}$z = new (get_class($m))(); }'''),
("first_class_callable_method", H + r'''
function f(Foo $m): void { ‸{Closure}$c = $m->foo(...); ‸{code: string $r}$r = $c(); }'''),
("first_class_callable_function", H + r'''
function mk(): Foo { return new Foo; }
function f(): void { $c = mk(...); ‸{code: Foo $r}$r = $c(); }'''),
("closure_fromcallable", H + r'''
function mk(): Foo { return new Foo; }
function f(): void { $c = \Closure::fromCallable('App\mk'); ‸{Closure}$c; }'''),
("callable_array_goto", H + r'''
class K { public function run(): void {} public static function srun(): void {} }
function f(K $k): void { array_map([$k, '‸{method run}run'], []); array_map([K::class, '‸{method srun}srun'], []); }'''),
("callable_string_goto", H + r'''
class K { public static function srun(): void {} }
function helper(): void {}
function f(): void { array_map('App\K::‸{method srun}srun', []); array_map('App\‸{function helper}helper', []); }'''),
("call_user_func_return", H + r'''
function mk(): Foo { return new Foo; }
function f(Foo $o): void { ‸{code: string $x}$x = call_user_func([$o, 'foo']); ‸{code: Foo $y}$y = call_user_func('App\mk'); }'''),
("dynamic_prop_name", H + r'''
function f(Foo $o, string $n): void { ‸{!error}$x = $o->$n; $o->{$n}; }'''),
("variable_variable", H + r'''
function f(): void { $name = 'v'; $$name = 1; }'''),
("compact_extract_highlight", H + r'''
function f(): array { ‸{variable foo}$foo = 1; return compact('foo'); }'''),
("compact_goto", H + r'''
function f(): array { $foo = 1; return compact('‸{variable foo}foo'); }'''),
("enum_cases_from", r'''namespace App;
enum Suit: string { case H = 'h'; case S = 's'; public function label(): string { return ''; } }
function f(): void {
    ‸{Suit}$a = Suit::cases();
    ‸{code: Suit $b}$b = Suit::from('h');
    ‸{code: ?Suit $c}$c = Suit::tryFrom('h');
    ‸{code: string $d}$d = Suit::H->value;
    ‸{code: string $e}$e = Suit::H->name;
    Suit::H->‸{label}label();
}'''),
("enum_pure_name", r'''namespace App;
enum E { case A; }
function f(E $e): void { ‸{code: string $n}$n = $e->name; }'''),
("enum_int_value", r'''namespace App;
enum E: int { case A = 1; }
function f(E $e): void { ‸{code: int $n}$n = $e->value; }'''),
("enum_const_case_hover", r'''namespace App;
enum E: int { case A = 1; const DEFAULT = self::A; }
function f(): void { ‸{code: E $n}$n = E::DEFAULT; }'''),
("stdclass_dynamic", H + r'''
function f(): void { $o = new \stdClass(); $o->a = 1; ‸{!error}$x = $o->a; }'''),
("object_cast_array", H + r'''
function f(): void { ‸{stdClass}$o = (object) ['a' => 1]; }'''),
("json_decode_obj", H + r'''
function f(string $j): void { ‸{stdClass}$o = json_decode($j); }'''),
("readonly_promoted_hover", H + r'''
class P { public function __construct(public readonly Foo $foo) {} }
function f(P $p): void { ‸{code: Foo $x}$x = $p->foo; $p->‸{readonly}foo; }'''),
("parent_static_call", H + r'''
class A { public function m(): Foo { return new Foo; } }
class B extends A { public function m(): Foo { ‸{code: Foo $x}$x = parent::m(); return $x; } }'''),
("static_prop_late", H + r'''
class A { /** @var Foo */ protected static $inst; public static function g(): void { ‸{code: Foo $x}$x = static::$inst; } }'''),
("closure_bind_this", H + r'''
function f(): void { $c = function () { return $this->foo(); }; \Closure::bind($c, new Foo, Foo::class); }'''),
("closure_bindto_this_hover", H + r'''
/** @param-closure-this Foo $c */
function run(\Closure $c): void {}
function f(): void { run(function () { ‸{code: string $x}$x = $this->foo(); }); }'''),
("isset_magic", H + r'''
/** @property Foo|null $p */
class Bag { public function __get($k) {} public function __isset($k): bool { return true; } }
function f(Bag $b): void { if (isset($b->p)) { ‸{code: Foo $x}$x = $b->p; } }'''),
("undefined_prop_without_get", H + r'''
function f(Foo $o): void { $o->nope; }'''),
("undefined_method_with_call", H + r'''
class Bag { public function __call($m, $a) {} }
function f(Bag $b): void { $b->nope(); }'''),
("undefined_static_without_callstatic", H + r'''
class Bag { public function __call($m, $a) {} }
function f(): void { Bag::nope(); }'''),
("spl_object_storage", H + r'''
/** @param \SplObjectStorage<Foo, int> $s */
function f(\SplObjectStorage $s): void { foreach ($s as ‸{code: Foo $o}$o) {} }'''),
("weakmap", H + r'''
/** @param \WeakMap<Foo, int> $s */
function f(\WeakMap $s, Foo $k): void { ‸{code: int $v}$v = $s[$k]; }'''),
("countable_count", H + r'''
class C implements \Countable { public function count(): int { return 0; } }
function f(C $c): void { ‸{code: int $n}$n = count($c); }'''),
("list_destructure_arrayaccess", H + r'''
/** @return array{Foo, int} */
function pair(): array { return [new Foo, 1]; }
function f(): void { [‸{code: Foo $a}$a, ‸{code: int $b}$b] = pair(); }'''),
("attribute_goto", H + r'''
#[\Attribute] class Route { public function __construct(public string $p) {} }
class Ctl { #[‸{class Route}Route('/x')] public function a(): void {} }'''),
("include_path_goto", r'''namespace App;
require_once __DIR__ . '/‸{a.php}a.php';'''),
("self_in_closure_static", H + r'''
class A { public function m(): void { $f = static fn() => new self(); ‸{code: A $x}$x = $f(); } }'''),
("array_callable_invoke_return", H + r'''
function f(Foo $o): void { $c = [$o, 'foo']; ‸{code: string $x}$x = $c(); }'''),
("string_callable_invoke", H + r'''
function mk(): Foo { return new Foo; }
function f(): void { $c = 'App\mk'; ‸{code: Foo $x}$x = $c(); }'''),
("magic_method_hover_tostring", H + r'''
class S { /** Render it. */ public function __toString(): string { return ''; } }
function f(S $s): void { echo $s->‸{__toString}__toString(); }'''),
("serialize_hooks", H + r'''
class S { public function __serialize(): array { return []; } public function __unserialize(array $d): void {} }
function f(S $s): void { ‸{S}$x = unserialize(serialize($s)); }'''),
("property_hook", r'''namespace App;
class P { public string $full { get => 'a'; } }
function f(P $p): void { ‸{code: string $x}$x = $p->full; }'''),
("asymmetric_visibility", r'''namespace App;
class P { public private(set) int $n = 0; }
function f(P $p): void { $p->n = 2; }'''),
("static_closure_callable_type", H + r'''
/** @param callable(int): Foo $cb */
function f(callable $cb): void { ‸{code: Foo $x}$x = $cb(1); }'''),
("closure_type_param", H + r'''
/** @param \Closure(int): Foo $cb */
function f(\Closure $cb): void { ‸{code: Foo $x}$x = $cb(1); }'''),
("new_anonymous_class", H + r'''
function f(): void { $o = new class extends Foo {}; ‸{code: string $x}$x = $o->foo(); }'''),
("instanceof_this", H + r'''
class A { public function m(): void { if ($this instanceof Sub) { ‸{code: int $x}$x = $this->s(); } } }
class Sub extends A { public function s(): int { return 1; } }'''),
("self_const_expr", H + r'''
class A { const X = [1, 2]; public function m(): void { ‸{array}$x = self::X; ‸{code: int $y}$y = static::X[0]; } }'''),
("global_const_define", H + r'''
define('MY_C', 'str');
const OTHER = 5;
function f(): void { ‸{code: string $x}$x = MY_C; ‸{code: int $y}$y = OTHER; }'''),
("unary_minus", H + r'''
function f(int $i): void { ‸{code: int $x}$x = -$i; ‸{code: int $y}$y = -1; }'''),
("increment_value", H + r'''
function f(int $i): void { ‸{code: int $x}$x = $i++; ‸{code: int $y}$y = ~$i; }'''),
("func_get_args", H + r'''
function f(): void { ‸{array}$x = func_get_args(); }'''),
("static_var", H + r'''
function f(): void { static $cache = []; ‸{array}$cache; }'''),
("global_var", H + r'''
function f(): void { global $db; }'''),
("superglobal_hover", H + r'''
function f(): void { ‸{array}$x = $_GET; ‸{string}$y = $_SERVER['REQUEST_URI']; }'''),
("heredoc_nowdoc", H + r'''
function f(): void { ‸{code: string $x}$x = <<<'T'
abc
T; }'''),
("match_expr", H + r'''
function f(int $i): void { ‸{Foo|string}$x = match ($i) { 1 => new Foo, default => 'a' }; }'''),
("throw_expr_null_coalesce", H + r'''
function f(?Foo $i): void { ‸{code: Foo $x}$x = $i ?? throw new \Exception(); }'''),
("nullsafe_magic", H + r'''
/** @property Foo $p */
class Bag { public function __get($k) {} }
function f(?Bag $b): void { ‸{?Foo}$x = $b?->p; }'''),
("d_var", H + r'''
/** @return array{Foo, int} */
function pair(): array { return [new Foo, 1]; }
function f(): void { $p = pair(); [‸{Foo $a}$a, $b] = $p; ‸{Foo $c}$c = $p[0]; }'''),
("d_keyed", H + r'''
/** @return array{a: Foo, b: int} */
function pair(): array { return ['a' => new Foo, 'b' => 1]; }
function f(): void { ['a' => ‸{Foo $a}$a] = pair(); }'''),
("d_list", H + r'''
/** @return array{Foo, int} */
function pair(): array { return [new Foo, 1]; }
function f(): void { list(‸{Foo $a}$a, $b) = pair(); }'''),
("d_list_of", H + r'''
/** @return list<Foo> */
function pair(): array { return [new Foo]; }
function f(): void { [‸{Foo $a}$a] = pair(); }'''),
("d_stmt_level", H + r'''
/** @return array{Foo, int} */
function pair(): array { return [new Foo, 1]; }
function f(): void {
    [$a, $b] = pair();
    ‸{Foo $z}$z = $a;
}'''),
("d_foreach", H + r'''
/** @param list<array{Foo, int}> $ps */
function f(array $ps): void { foreach ($ps as [‸{Foo $a}$a, $b]) {} }'''),
("throw_coalesce", H + r'''
function f(?Foo $i): void {
    $x = $i ?? throw new \Exception();
    ‸{code: Foo $y}$y = $x;
}'''),
("throw_ternary", H + r'''
function f(?Foo $i): void {
    ‸{code: Foo $x}$x = $i !== null ? $i : throw new \Exception();
}'''),
("throw_elvis", H + r'''
function f(Foo|false|null $i): void { ‸{code: Foo $x}$x = $i ?: throw new \Exception(); }'''),
("throw_array_default", H + r'''
function f(?array $d): array { ‸{code: array $x}$x = $d ?? throw new \Exception(); return $x; }'''),
# --- should be silent
("fp_invoke_as_callable", H + r'''
class Inv { public function __invoke(int $a): int { return $a; } }
function takes(callable $c): void {}
function f(Inv $i): void { takes($i); array_map($i, [1]); }'''),
("fp_invoke_as_closure_param", H + r'''
class Inv { public function __invoke(): void {} }
function takes(\Closure $c): void {}
function f(Inv $i): void { takes($i); }'''),
("fp_tostring_nonstrict", H + r'''
class S { public function __toString(): string { return ''; } }
function takes(string $s): void {}
function f(S $s): void { takes($s); echo $s; $x = 'a' . $s; strlen($s); }'''),
("fp_stringable_param", H + r'''
class S implements \Stringable { public function __toString(): string { return ''; } }
function takes(\Stringable|string $s): void {}
function f(S $s): void { takes($s); takes('x'); }'''),
("fp_traversable_iterable", H + r'''
function takes(iterable $s): void {}
function f(\ArrayIterator $s, \Generator $g): void { takes($s); takes($g); }'''),
("fp_countable_count", H + r'''
class C implements \Countable { public function count(): int { return 0; } }
function f(C $c): int { return count($c); }'''),
("fp_set_undeclared", H + r'''
class Bag { public function __set(string $k, mixed $v): void {} public function __get(string $k): mixed { return null; } }
function f(Bag $b): void { $b->anything = 1; unset($b->other); $x = isset($b->third); }'''),
("fp_call_undeclared", H + r'''
class Bag { public function __call(string $m, array $a): mixed { return null; } }
function f(Bag $b): void { $b->anything(1, 2); $b->a()?->b(); }'''),
("fp_callstatic_undeclared", H + r'''
class Facade { public static function __callStatic(string $m, array $a): mixed { return null; } }
function f(): void { Facade::anything(); }'''),
("fp_mixin_members", H + r'''
/** @mixin Foo */
class Proxy { public function __call($m, $a) {} public function __get($n) {} }
function f(Proxy $p): void { $p->foo(); $x = $p->id; }'''),
("fp_arrayaccess_write_isset", H + r'''
class Coll implements \ArrayAccess {
    public function offsetExists(mixed $o): bool { return true; }
    public function offsetGet(mixed $o): Foo { return new Foo; }
    public function offsetSet(mixed $o, mixed $v): void {}
    public function offsetUnset(mixed $o): void {}
}
function f(Coll $c): void { $c['k'] = new Foo; $c[] = new Foo; unset($c['k']); $x = isset($c['k']); $y = $c['k'] ?? null; }'''),
("fp_dynamic_method_name", H + r'''
function f(Foo $o, string $m): void { $o->$m(); $o->{$m . 'x'}(); [$o, $m](); }'''),
("fp_static_call_instance_parent", H + r'''
class A { public function m(): int { return 1; } }
class B extends A { public function m(): int { return parent::m() + A::m(); } }'''),
("fp_enum_interface_consts", r'''namespace App;
interface HasLabel { public function label(): string; }
enum E: string implements HasLabel { case A = 'a'; const D = self::A; public function label(): string { return $this->value; } public static function fromLabel(string $l): self { return self::from($l); } }
function f(): void { $x = E::D->label(); $y = E::A instanceof HasLabel; foreach (E::cases() as $c) { $c->label(); } }'''),
("fp_closure_this_bound", H + r'''
class Host { private int $secret = 1; }
function f(): void { $peek = function () { return $this->secret; }; $peek->call(new Host); \Closure::bind($peek, new Host, Host::class)(); }'''),
("fp_generator_methods", H + r'''
/** @return \Generator<int, Foo, mixed, string> */
function gen(): \Generator { yield new Foo; return 'x'; }
function f(): void { $g = gen(); $g->current()?->foo(); $g->send(1); $r = $g->getReturn(); strlen($r); }'''),
("fp_serializable_magic", H + r'''
class S { public function __serialize(): array { return []; } public function __unserialize(array $d): void {} public function __sleep(): array { return []; } public function __wakeup(): void {} public function __clone(): void {} public function __debugInfo(): array { return []; } public static function __set_state(array $a): static { return new static; } }
function f(S $s): void { $c = clone $s; var_export($s); }'''),
("fp_property_exists_guard", H + r'''
function f(object $o): void { if (property_exists($o, 'name')) { echo $o->name; } if (method_exists($o, 'run')) { $o->run(); } }'''),
("fp_readonly_clone_with", r'''namespace App;
final class P { public function __construct(public readonly int $n) {} public function with(int $n): static { return clone($this, ['n' => $n]); } }'''),
("fp_stdclass_props", H + r'''
function f(): void { $o = new \stdClass; $o->a = 1; $o->b->c = 2; echo $o->a, $o->zzz; }'''),
("fp_simplexml", H + r'''
function f(\SimpleXMLElement $x): void { echo $x->item->name; foreach ($x->item as $i) { echo (string) $i['attr']; } }'''),
# --- should report
("tp_property_read_write", H + r'''
/** @property-read int $r */
class Bag { public function __get($k) {} public function __set($k, $v) {} }
function f(Bag $b): void { $b->r = 5; }'''),
("tp_undefined_without_magic", H + r'''
function f(Foo $o): void { $o->nope(); $o->nope; Foo::nope(); }'''),
("tp_static_call_only_call", H + r'''
class Bag { public function __call($m, $a) {} }
function f(): void { Bag::nope(); }'''),
("tp_invoke_missing", H + r'''
function f(Foo $o): void { $o(); }'''),
("tp_tostring_missing", H + r'''
function takes(string $s): void {}
function f(Foo $o): void { takes($o); echo $o; }'''),
("tp_arrayaccess_missing", H + r'''
function f(Foo $o): void { $x = $o['k']; }'''),
("tp_iterate_non_traversable", H + r'''
function f(Foo $o): void { foreach ($o as $v) {} }'''),
("tp_enum_from_on_pure", r'''namespace App;
enum E { case A; }
function f(): void { E::from('a'); $x = E::A->value; }'''),
("tp_new_abstract_interface", H + r'''
abstract class A {} interface I {}
function f(): void { new A(); new I(); }'''),
("tp_strict_tostring", r'''declare(strict_types=1);
namespace App;
class S { public function __toString(): string { return ''; } }
function takes(string $s): void {}
function f(S $s): void { takes($s); }'''),
# --- navigation / completion
("nav_goto_magic_prop", H + r'''
/** @property Foo $p */
class Bag { public function __get($k) {} }
function f(Bag $b): void { $b->‸{property p}p; }'''),
("nav_goto_via_get", H + r'''
class Bag { public function __get(string $k): Foo { return new Foo; } }
function f(Bag $b): void { $b->‸{__get}any; }'''),
("nav_goto_via_call", H + r'''
class Bag { public function __call(string $k, array $a): Foo { return new Foo; } }
function f(Bag $b): void { $b->‸{__call}any(); }'''),
("nav_goto_mixin", H + r'''
/** @mixin Foo */
class Proxy { public function __call($m, $a) {} }
function f(Proxy $p): void { $p->‸{method foo}foo(); }'''),
("nav_hover_method_tag", H + r'''
/** @method Foo make(int $n) Make one. */
class Bag { public function __call($m, $a) {} }
function f(Bag $b): void { $b->‸{make(int $n): Foo}make(1); }'''),
("nav_completion_mixin", H + r'''
/** @mixin Foo */
class Proxy { public function __call($m, $a) {} public function own(): void {} }
function f(Proxy $p): void { $p->‸{own&&foo}own(); }'''),
("nav_completion_enum_static", r'''namespace App;
enum E: int { case A = 1; case B = 2; }
function f(): void { E::‸{A&&B&&cases&&from&&tryFrom}A; }'''),
("nav_completion_enum_instance", r'''namespace App;
enum E: int { case A = 1; }
function f(E $e): void { $e->‸{name&&value}name; }'''),
("nav_invoke_goto", H + r'''
class Inv { public function __invoke(): Foo { return new Foo; } }
function f(Inv $i): void { ‸{variable i}$i(); }'''),
("nav_new_class_string_goto", H + r'''
function f(): void { $c = Foo::class; $o = new $c(); $o->‸{method foo}foo(); }'''),
("nav_static_method_via_instance", H + r'''
class M { public static function make(): static { return new static; } }
function f(M $m): void { $x = $m::make(); $x->‸{make}make(); }'''),
# A fourth pass: traits, generics reached with nothing supplied, callbacks that are not closures.
('trait_alias_as', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\ntrait A { public function hello(): Foo { return new Foo; } }\ntrait B { public function hello(): int { return 1; } }\nclass C { use A, B { A::hello insteadof B; B::hello as helloB; } }\nfunction f(C $c): void { ‸{code: Foo $x}$x = $c->hello(); ‸{code: int $y}$y = $c->helloB(); $c->‸{helloB}helloB(); }"),
('trait_alias_visibility', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\ntrait A { private function secret(): Foo { return new Foo; } }\nclass C { use A { secret as public revealed; } }\nfunction f(C $c): void { ‸{code: Foo $x}$x = $c->revealed(); }"),
('trait_static_self', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\ntrait Make { public static function make(): static { return new static; } public function me(): self { return $this; } }\nclass C { use Make; public function own(): int { return 1; } }\nfunction f(): void { ‸{code: C $x}$x = C::make(); ‸{code: C $y}$y = C::make()->me(); }"),
('trait_abstract_and_property', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\ntrait T { protected Foo $dep; abstract public function need(): Foo; public function use(): string { return $this->dep->foo() . $this->need()->foo(); } }\nclass C { use T; public function need(): Foo { return new Foo; } }\nfunction f(C $c): void { ‸{code: string $x}$x = $c->use(); }"),
('static_first_class_callable', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass M { public static function make(): Foo { return new Foo; } }\nfunction f(): void { $c = M::make(...); ‸{code: Foo $x}$x = $c(); }"),
('new_first_class_on_this', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass M { public function make(): Foo { return new Foo; } public function g(): void { $c = $this->make(...); ‸{code: Foo $x}$x = $c(); } }"),
('yield_from', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @return \\Generator<int, Foo> */\nfunction inner(): \\Generator { yield new Foo; }\nfunction outer(): \\Generator { yield from inner(); }\nfunction f(): void { foreach (inner() as ‸{code: Foo $v}$v) {} }"),
('generic_collection_new', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @template T */\nclass Collection implements \\IteratorAggregate {\n    /** @param array<int, T> $items */\n    public function __construct(private array $items) {}\n    /** @return T|null */\n    public function first(): mixed { return $this->items[0] ?? null; }\n    /** @return \\ArrayIterator<int, T> */\n    public function getIterator(): \\ArrayIterator { return new \\ArrayIterator($this->items); }\n}\nfunction f(): void {\n    $c = new Collection([new Foo]);\n    ‸{code: ?Foo $x}$x = $c->first();\n    foreach ($c as ‸{code: Foo $v}$v) {}\n}"),
('generic_var_annotation', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @template T */\nclass Box { /** @return T */ public function get(): mixed { return null; } }\nfunction f(): void { /** @var Box<Foo> $b */ $b = make(); ‸{code: Foo $x}$x = $b->get(); }\nfunction make(): Box { return new Box; }"),
('generic_extends', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @template T */\nabstract class Repo { /** @return T */ public function find(): mixed { return null; } }\n/** @extends Repo<Foo> */\nclass FooRepo extends Repo {}\nfunction f(FooRepo $r): void { ‸{code: Foo $x}$x = $r->find(); $r->find()->‸{foo}foo(); }"),
('generic_method_template', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass Container {\n    /**\n     * @template T\n     * @param class-string<T> $id\n     * @return T\n     */\n    public function get(string $id): object { return new $id; }\n}\nfunction f(Container $c): void { ‸{code: Foo $x}$x = $c->get(Foo::class); }"),
('arrow_fn_param_from_array_map', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param list<Foo> $l */\nfunction f(array $l): void { array_map(fn($x) => ‸{code: Foo $x}$x->foo(), $l); array_filter($l, function ($y) { return ‸{code: Foo $y}$y->id > 0; }); }"),
('usort_callback_params', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param list<Foo> $l */\nfunction f(array $l): void { usort($l, fn($a, $b) => ‸{code: Foo $a}$a->id <=> $b->id); }"),
('closure_use_by_ref', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(): void { $x = new Foo; $g = function () use ($x) { return ‸{code: Foo $x}$x; }; }"),
('match_true', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(Foo|string|null $v): void { $r = match (true) { $v instanceof Foo => ‸{code: Foo $v}$v, default => null }; }"),
('nullsafe_chain', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass A { public function b(): ?Foo { return null; } }\nfunction f(?A $a): void { ‸{code: ?string $x}$x = $a?->b()?->foo(); }"),
('named_args_magic', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @method Foo make(int $n, string $label = '') */\nclass Bag { public function __call($m, $a) {} }\nfunction f(Bag $b): void { ‸{code: Foo $x}$x = $b->make(label: 'x', n: 1); }"),
('property_hook_set', "namespace App;\nclass P { public string $name { set(string $v) { $this->name = strtolower($v); } } }\nfunction f(P $p): void { $p->name = 'X'; ‸{code: string $x}$x = $p->name; }"),
('interface_constant_via_instance', 'namespace App;\ninterface HasV { const V = 3; }\nclass C implements HasV {}\nfunction f(C $c): void { ‸{code: int $x}$x = $c::V; ‸{code: int $y}$y = C::V; }'),
('enum_const_expr', "namespace App;\nenum E: string { case A = 'a'; case B = 'b'; const ALL = [self::A, self::B]; }\nfunction f(): void { foreach (E::ALL as ‸{code: E $e}$e) {} }"),
('enum_cases_foreach', "namespace App;\nenum E: string { case A = 'a'; }\nfunction f(): void { foreach (E::cases() as ‸{code: E $e}$e) { $e->‸{value}value; } }"),
('enum_from_value', "namespace App;\nenum E: string { case A = 'a'; }\nfunction f(string $s): void { ‸{code: string $v}$v = E::from($s)->value; }"),
('instanceof_string_class', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(object $o): void { $c = Foo::class; if ($o instanceof $c) { ‸{code: Foo $o}$o; } }"),
('self_instanceof_this_static', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass A { public function eq(self $other): bool { return $other instanceof static; } public static function make(): static { ‸{static}$x = new static(); return $x; } }"),
('array_shape_foreach_keys', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param array<string, Foo> $m */\nfunction f(array $m): void { foreach ($m as ‸{code: string $k}$k => $v) {} }"),
('iterator_to_array_generic', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param \\Traversable<int, Foo> $t */\nfunction f(\\Traversable $t): void { foreach (iterator_to_array($t) as ‸{code: Foo $v}$v) {} }"),
('array_values_keep_type', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param array<string, Foo> $m */\nfunction f(array $m): void { ‸{Foo}$l = array_values($m); ‸{code: ?Foo $first}$first = array_values($m)[0] ?? null; }"),
('reset_end_current', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param non-empty-list<Foo> $l */\nfunction f(array $l): void { ‸{Foo}$a = reset($l); ‸{Foo}$b = end($l); ‸{Foo}$c = current($l); }"),
('array_key_first_last', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param array<string, Foo> $m */\nfunction f(array $m): void { ‸{code: ?string $k}$k = array_key_first($m); }"),
('list_each_in_foreach_key', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param list<array{id: int, foo: Foo}> $rows */\nfunction f(array $rows): void { foreach ($rows as ['id' => ‸{code: int $id}$id, 'foo' => ‸{code: Foo $foo}$foo]) {} }"),
('clone_with_modify', 'namespace App;\nfinal class P { public function __construct(public readonly int $n) {} }\nfunction f(P $p): void { ‸{code: P $q}$q = clone $p; }'),
('static_closure_in_class', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass A { public function m(): void { $f = static fn(): Foo => new Foo; ‸{code: Foo $x}$x = $f(); } }"),
('closure_from_callable_method', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(Foo $o): void { $c = \\Closure::fromCallable([$o, 'foo']); ‸{code: string $x}$x = $c(); }"),
('invokable_class_string_new', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass Inv { public function __invoke(): Foo { return new Foo; } }\nfunction f(): void { ‸{code: Foo $x}$x = (new Inv)(); }"),
('array_callable_in_array_map_return', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass K { public function conv(int $i): Foo { return new Foo; } public function g(): void { ‸{Foo}$r = array_map([$this, 'conv'], [1]); } }"),
('parent_constructor_goto', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass A { public function __construct(int $x) {} }\nclass B extends A { public function __construct() { parent::‸{method __construct}__construct(1); } }"),
('new_self_in_static', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass A { public static function one(): self { ‸{code: A $x}$x = new self(); return $x; } }"),
('abstract_static_call', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nabstract class A { abstract public static function make(): static; public static function build(): static { ‸{static}$x = static::make(); return $x; } }"),
('fp_undefined_static_prop_late', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass A { protected static array $reg = []; public static function add(): void { static::$reg[] = 1; self::$reg[] = 2; } }"),
('fp_constructor_promotion_readonly_write_in_ctor', 'namespace App;\nclass P { public function __construct(public readonly int $n) { } public static function of(int $n): static { return new static($n); } }'),
('fp_trait_insteadof_undefined', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\ntrait A { public function x(): int { return 1; } }\ntrait B { public function x(): int { return 2; } }\nclass C { use A, B { A::x insteadof B; B::x as y; } }\nfunction f(C $c): void { $c->x(); $c->y(); }"),
('fp_interface_static_call', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\ninterface Factory { public static function create(): static; }\nfunction f(string $cls): void { /** @var class-string<Factory> $cls */ $cls::create(); }"),
('fp_magic_constant_in_trait', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\ntrait T { public function name(): string { return __CLASS__ . self::class . static::class . __TRAIT__; } }"),
('brackets_literal', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @template T */\nclass Coll {\n    /** @param T[] $items */\n    public function __construct(private array $items) {}\n    /** @return T */\n    public function first(): mixed { return $this->items[0]; }\n}\nfunction f(): void { $c = new Coll([new Foo]); ‸{code: Foo $x}$x = $c->first(); }"),
('generic_literal', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @template T */\nclass Coll {\n    /** @param array<int, T> $items */\n    public function __construct(private array $items) {}\n    /** @return T */\n    public function first(): mixed { return $this->items[0]; }\n}\nfunction f(): void { $c = new Coll([new Foo]); ‸{code: Foo $x}$x = $c->first(); }"),
('list_literal', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @template T */\nclass Coll {\n    /** @param list<T> $items */\n    public function __construct(private array $items) {}\n    /** @return T */\n    public function first(): mixed { return $this->items[0]; }\n}\nfunction f(): void { $c = new Coll([new Foo, new Foo]); ‸{code: Foo $x}$x = $c->first(); }"),
('generic_typed_var', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @template T */\nclass Coll {\n    /** @param array<int, T> $items */\n    public function __construct(private array $items) {}\n    /** @return T */\n    public function first(): mixed { return $this->items[0]; }\n}\n/** @param list<Foo> $l */ function f(array $l): void { $c = new Coll($l); ‸{code: Foo $x}$x = $c->first(); }"),
('brackets_typed_var', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @template T */\nclass Coll {\n    /** @param T[] $items */\n    public function __construct(private array $items) {}\n    /** @return T */\n    public function first(): mixed { return $this->items[0]; }\n}\n/** @param list<Foo> $l */ function f(array $l): void { $c = new Coll($l); ‸{code: Foo $x}$x = $c->first(); }"),
('const_map', "namespace App;\n\nclass T {\n    const SIZES = ['thumb' => 200, 'full' => 1200];\n    const NAMES = ['a', 'b'];\n    const FLAGS = ['x' => true, 'y' => false];\n    const MIXED = ['a' => 1, 'b' => 'x'];\n    const NESTED = ['a' => ['x' => 1]];\n    const NUMERIC = ['1' => 'a', '2' => 'b'];\n    public function m(string $k): void {\n        ‸{code: int $a}$a = self::SIZES[$k];\n        foreach (static::NAMES as ‸{code: string $n}$n) {}\n        ‸{code: bool $f}$f = self::FLAGS['x'];\n        ‸{!int&&!string $x}$x = self::MIXED['a'];\n        ‸{code: array $y}$y = self::NESTED['a'];\n        foreach (self::NUMERIC as ‸{code: int $key}$key => $v) {}\n        ‸{code: array<string, int> $whole}$whole = self::SIZES;\n    }\n}"),
('array_map_forms', "namespace App;\nclass Foo { public function foo(): string { return ''; } }\n\nclass K {\n    public function conv(int $i): Foo { return new Foo; }\n    public static function sconv(int $i): Foo { return new Foo; }\n    /** @param list<int> $l */\n    public function g(array $l): void {\n        foreach (array_map([$this, 'conv'], $l) as ‸{code: Foo $a}$a) {}\n        foreach (array_map($this->conv(...), $l) as ‸{code: Foo $b}$b) {}\n        foreach (array_map(fn($i) => new Foo, $l) as ‸{code: Foo $c}$c) {}\n        foreach (array_map([self::class, 'sconv'], $l) as ‸{code: Foo $d}$d) {}\n        foreach (array_map(self::sconv(...), $l) as ‸{code: Foo $e}$e) {}\n        foreach (array_map('App\\mk', $l) as ‸{code: Foo $f}$f) {}\n        ‸{code: Foo $g}$g = call_user_func([$this, 'conv'], 1);\n        ‸{code: Foo $h}$h = call_user_func_array([$this, 'conv'], [1]);\n        $c = [$this, 'conv']; ‸{code: Foo $i}$i = $c(1);\n        ‸{code: Foo $j}$j = \\Closure::fromCallable([$this, 'conv'])(1);\n    }\n}\nfunction mk(int $i): Foo { return new Foo; }"),
('merge_spread', 'namespace App;\n\n/**\n * @param list<list<string>> $groups\n * @param list<string> $a\n * @param list<string> $b\n */\nfunction f(array $groups, array $a, array $b): void {\n    foreach (array_merge(...$groups) as ‸{code: string $s}$s) {}\n    foreach (array_merge($a, $b) as ‸{code: string $t}$t) {}\n}'),
('reduce_carry', 'namespace App;\n\n/** @param list<string> $names */\nfunction f(array $names): void {\n    array_reduce($names, static fn (array $carry, string $n) => ‸{code: array $carry}$carry, []);\n    array_reduce($names, fn ($carry, $n) => ‸{!string}$carry, []);\n}'),
('merge_two', 'namespace App;\n\n/**\n * @param list<string> $a\n * @param list<string> $b\n */\nfunction f(array $a, array $b): void {\n    foreach (array_merge($a, $b) as ‸{code: string $t}$t) {}\n}'),
('method_template_multiline', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass Container {\n    /**\n     * @template T\n     * @param class-string<T> $id\n     * @return T\n     */\n    public function get(string $id): object { return new $id; }\n}\nfunction f(Container $c): void { ‸{code: Foo $x}$x = $c->get(Foo::class); }"),
('ctor_template_from_param', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @template T */\nclass Box {\n    /** @param T $v */\n    public function __construct(private mixed $v) {}\n    /** @return T */\n    public function get(): mixed { return $this->v; }\n}\nfunction f(): void { $b = new Box(new Foo); ‸{code: Foo $x}$x = $b->get(); }"),
('ctor_template_from_array', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @template T */\nclass Coll {\n    /** @param array<int, T> $items */\n    public function __construct(private array $items) {}\n    /** @return T */\n    public function first(): mixed { return $this->items[0]; }\n}\nfunction f(): void { $c = new Coll([new Foo]); ‸{code: Foo $x}$x = $c->first(); }"),
('combine', "namespace App;\n\n/** @param list<string> $names */\nfunction f(array $names): void {\n    $m = array_combine($names, array_map('strlen', $names));\n    foreach ($m as ‸{code: string $k}$k => ‸{code: int $v}$v) {}\n    foreach (iterator_to_array(new \\ArrayIterator(['a' => 1])) as $x => $y) {}\n    foreach (array_values($m) as ‸{code: int $i}$i => $w) {}\n}"),
# A fifth pass: generators, invokables as callbacks, narrowing on properties, array_column.
('getiterator_yield_infer', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass Coll implements \\IteratorAggregate { public function getIterator(): \\Generator { yield new Foo; } }\nfunction f(Coll $c): void { foreach ($c as ‸{code: Foo $v}$v) {} }"),
('getiterator_yield_doc', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass Coll implements \\IteratorAggregate { /** @return \\Generator<int, Foo> */ public function getIterator(): \\Generator { yield new Foo; } }\nfunction f(Coll $c): void { foreach ($c as ‸{code: Foo $v}$v) {} }"),
('generator_infer_from_body', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction gen() { yield new Foo; }\nfunction f(): void { foreach (gen() as ‸{code: Foo $v}$v) {} }"),
('invokable_as_callback', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass Conv { public function __invoke(int $i): Foo { return new Foo; } }\n/** @param list<int> $l */\nfunction f(array $l, Conv $c): void { foreach (array_map($c, $l) as ‸{code: Foo $v}$v) {} foreach (array_map(new Conv, $l) as ‸{code: Foo $w}$w) {} }"),
('interface_static_return', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\ninterface Make { public static function make(): static; }\nfinal class M implements Make { public static function make(): static { return new static; } public function own(): int { return 1; } }\nfunction f(): void { ‸{code: M $x}$x = M::make(); }"),
('static_via_interface_var', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\ninterface Fluent { public function with(): static; }\nclass Impl implements Fluent { public function with(): static { return $this; } public function extra(): int { return 1; } }\nfunction f(Impl $i): void { ‸{code: Impl $x}$x = $i->with(); }"),
('instanceof_property', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass Holder { public ?object $item = null;\n  public function m(): void { if ($this->item instanceof Foo) { ‸{code: string $x}$x = $this->item->foo(); } } }"),
('instanceof_property_early_return', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass Holder { public ?object $item = null;\n  public function m(): string { if (!$this->item instanceof Foo) { return ''; } return ‸{code: Foo $y}$y = $this->item; } }"),
('never_function_narrows', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction fail(): never { throw new \\Exception(); }\nfunction f(?Foo $x): void { if ($x === null) { fail(); } ‸{code: Foo $y}$y = $x; }"),
('match_no_default', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(int $i): void { ‸{code: Foo|string $x}$x = match ($i) { 1 => new Foo, 2 => 'a' }; }"),
('catch_multi', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass AEx extends \\Exception { public function a(): int { return 1; } }\nclass BEx extends \\Exception { public function a(): int { return 2; } }\nfunction f(): void { try {} catch (AEx|BEx $e) { ‸{code: int $x}$x = $e->a(); ‸{AEx|BEx}$e; } }"),
('iterable_param', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param iterable<Foo> $it */\nfunction f(iterable $it): void { foreach ($it as ‸{code: Foo $v}$v) {} }"),
('iterable_key', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param iterable<string, Foo> $it */\nfunction f(iterable $it): void { foreach ($it as ‸{code: string $k}$k => $v) {} }"),
('static_property_array_elem', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass Reg { /** @var array<string, Foo> */ private static array $items = [];\n  public static function get(string $k): void { ‸{code: Foo $x}$x = self::$items[$k]; } }"),
('self_new_in_initializer', 'namespace App;\nclass Dep { public function d(): int { return 1; } }\nclass S { public function __construct(private Dep $dep = new Dep()) {} public function m(): void { ‸{code: int $x}$x = $this->dep->d(); } }'),
('promoted_readonly_class', 'namespace App;\nreadonly class P { public function __construct(public int $n, public ?P $next = null) {} }\nfunction f(P $p): void { ‸{code: ?P $x}$x = $p->next; ‸{code: int $y}$y = $p->next?->n ?? 0; }'),
('closure_this_in_class', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass A { private Foo $foo; public function m(): void { $f = function () { return $this->foo; }; ‸{code: Foo $x}$x = $f(); } }"),
('arrow_captures_outer', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(Foo $o): void { $g = fn() => $o->foo(); ‸{code: string $x}$x = $g(); }"),
('list_nested_keys', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @return array{a: array{b: Foo}} */\nfunction g(): array { return ['a' => ['b' => new Foo]]; }\nfunction f(): void { ['a' => ['b' => ‸{code: Foo $x}$x]] = g(); }"),
('yield_key_value', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @return \\Generator<string, Foo> */\nfunction g(): \\Generator { yield 'k' => new Foo; }\nfunction f(): void { foreach (g() as ‸{code: string $k}$k => ‸{code: Foo $v}$v) {} }"),
('array_filter_keeps', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param list<?Foo> $l */\nfunction f(array $l): void { foreach (array_filter($l) as ‸{code: Foo $v}$v) {} }"),
('array_column', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param list<array{id: int, name: string}> $rows */\nfunction f(array $rows): void { foreach (array_column($rows, 'name') as ‸{code: string $n}$n) {} }"),
('array_key_exists_narrow', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param array{a?: Foo} $m */\nfunction f(array $m): void { if (array_key_exists('a', $m)) { ‸{code: Foo $x}$x = $m['a']; } }"),
('isset_array_narrow', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\n/** @param array<string, ?Foo> $m */\nfunction f(array $m, string $k): void { if (isset($m[$k])) { ‸{code: Foo $x}$x = $m[$k]; } }"),
('in_array_strict_narrow', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(string $s): void { if (in_array($s, ['a', 'b'], true)) { ‸{string}$s; } }"),
('str_contains_narrow_no', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(?string $s): void { if ($s !== null && str_contains($s, 'x')) { ‸{code: string $t}$t = $s; } }"),
('instanceof_interface_then_method', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\ninterface HasName { public function name(): string; }\nfunction f(object $o): void { if ($o instanceof HasName) { ‸{code: string $x}$x = $o->name(); } }"),
('assert_instanceof', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(mixed $o): void { assert($o instanceof Foo); ‸{code: Foo $x}$x = $o; }"),
('is_a_narrow', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(object $o): void { if (is_a($o, Foo::class)) { ‸{code: Foo $x}$x = $o; } }"),
('class_exists_new', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(string $c): void { if (class_exists($c)) { $o = new $c(); } }"),
('fp_parent_construct_named', "namespace App;\nclass A { public function __construct(protected int $n = 0, protected string $s = '') {} }\nclass B extends A { public function __construct() { parent::__construct(s: 'x'); } }"),
('fp_abstract_static', 'namespace App;\nabstract class A { abstract protected static function key(): string; public static function k(): string { return static::key(); } }'),
('fp_interface_constant_self', "namespace App;\ninterface I { const X = 'a'; public function m(): string; }\nclass C implements I { public function m(): string { return self::X . static::X . I::X; } }"),
('fp_enum_in_match', "namespace App;\nenum S: string { case A = 'a'; case B = 'b'; public function label(): string { return match ($this) { self::A => 'A', self::B => 'B' }; } }"),
('fp_first_class_static', 'namespace App;\nclass M { public static function a(int $x): int { return $x; } public function g(): array { return array_map(static::a(...), [1]); } }'),
('fp_nullsafe_method_chain', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass A { public function b(): ?Foo { return null; } }\nfunction f(?A $a): string { return $a?->b()?->foo() ?? ''; }"),
('fp_spread_named', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction g(int $a, int $b): int { return $a + $b; }\nfunction f(): int { $args = ['a' => 1, 'b' => 2]; return g(...$args); }"),
('fp_static_prop_via_static', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nclass A { protected static ?Foo $i = null; public static function get(): Foo { return static::$i ??= new Foo; } }"),
('fp_readonly_init_in_ctor', 'namespace App;\nfinal class P { public readonly int $n; public function __construct() { $this->n = 1; } }'),
('fp_callable_string_param', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction takes(callable $c): void {}\nfunction f(): void { takes('strlen'); takes([new Foo, 'foo']); takes(fn() => 1); takes(Foo::class . '::x'); }"),
('fp_countable_and_iterator', "namespace App;\nclass Foo { public int $id = 1; public function foo(): string { return ''; } }\n\nfunction f(\\ArrayObject $a): int { $n = count($a); foreach ($a as $k => $v) {} return $n; }"),
('named_args_multiline', 'namespace App;\nclass Foo {}\n\n/**\n * @template T\n * @param class-string<T> $cls\n * @return T\n */\nfunction make(string $cls, int $n = 0): object { return new $cls; }\nfunction f(): void { ‸{code: Foo $x}$x = make(n: 1, cls: Foo::class); ‸{code: Foo $y}$y = make(Foo::class); }'),
('column', "namespace App;\nclass Foo { public string $label = ''; }\nenum E: string { case A = 'a'; }\n\n/** @param list<array{id: int, name: string}> $rows\n */\nfunction f(array $rows): void {\n    foreach (array_column($rows, 'name') as ‸{code: string $n}$n) {}\n    foreach (array_column($rows, 'id') as ‸{code: int $i}$i) {}\n}\n/** @param list<Foo> $objs */\nfunction g(array $objs, string $k): void {\n    foreach (array_column($objs, 'label') as ‸{code: string $l}$l) {}\n    foreach (array_column(E::cases(), 'value') as ‸{code: string $v}$v) {}\n}\n/** @param list<array{id: int, name: string}> $rows */\nfunction h(array $rows, string $k): void {\n    foreach (array_column($rows, $k) as ‸{code: int|string $x}$x) {}\n    foreach (array_column($rows, null, 'id') as ‸{name: string}$r) {}\n}"),
# A sixth pass, from PHPStan's level 8 findings on the reference project: compound assignments, class-name tables, guards that leave, plain container returns.
('compound_assign', "namespace App;\n\nfunction f(bool $b, int $i, string $s, array $a, float $x): void {\n    $r = true; $r &= $b; ‸{code: int $r1}$r1 = $r;\n    $n = 1; $n += 2; ‸{code: int $n1}$n1 = $n;\n    $f = 1; $f /= 2; ‸{int|float}$f1 = $f;\n    $t = 'a'; $t .= $i; ‸{code: string $t1}$t1 = $t;\n    $c = [1]; $c += [2]; ‸{array}$c1 = $c;\n    $m = null; $m ??= 'x'; ‸{code: string $m1}$m1 = $m;\n    $p = 2; $p **= 2; ‸{code: int $p1}$p1 = $p;\n    $o = 1; $o |= 4; ‸{code: int $o1}$o1 = $o;\n}"),
('loop_accum', "namespace App;\nclass Row { public float $amount = 0.0; }\n\n/** @param list<Row> $rows */\nfunction f(array $rows, bool $c): void {\n    $total = 0;\n    foreach ($rows as $r) { $total += $r->amount; }\n    ‸{float|int}$t = $total;\n    $s = '';\n    foreach ($rows as $r) { $s .= 'x'; }\n    ‸{code: string $u}$u = $s;\n    $flags = 0;\n    if ($c) { $flags |= 4; }\n    ‸{code: int $v}$v = $flags;\n    $n = null;\n    if ($c) { $n .= 'a'; }\n    ‸{?string}$w = $n;\n}"),
('class_table', "namespace App;\nabstract class App {}\nclass Order extends App { public function order(): int { return 1; } }\nclass Line extends App {}\n\nclass F {\n    const TYPES = ['order' => Order::class, 'line' => Line::class];\n    public const array TYPED = ['order' => Order::class];\n    public const array NAMES = ['a', 'b'];\n    public static function make(string $k): App {\n        $class = self::TYPES[$k];\n        ‸{Order|Line}$o = new $class();\n        $one = static::TYPED[$k];\n        ‸{code: Order $p}$p = new $one();\n        foreach (self::NAMES as ‸{code: string $n}$n) {}\n        return $o;\n    }\n}"),
('early_instance', "namespace App;\ninterface U {}\nclass Obj { public function has(): bool { return true; } }\n\nfunction f(Obj $o): void { if (!method_exists($o, 'extra')) { return; } $o->extra(); }"),
('early_or', "namespace App;\ninterface U {}\nclass Obj { public function has(): bool { return true; } }\n\nfunction f(Obj $o, $v): bool { if ($v === null || !method_exists($o, 'extra')) { return false; } $o->extra(); return true; }"),
('early_static_classstring', "namespace App;\ninterface U {}\nclass Obj { public function has(): bool { return true; } }\n\nclass R { /** @var class-string<U> */ protected string $type = U::class;\n  public function f($v): mixed { if (!method_exists($this->type, 'tryFrom')) { return false; } return $this->type::tryFrom($v); } }"),
('early_static_in_try', "namespace App;\ninterface U {}\nclass Obj { public function has(): bool { return true; } }\n\nclass R { /** @var class-string<U> */ protected string $type = U::class;\n  public function f($v): mixed { if (!method_exists($this->type, 'tryFrom')) { return false; } try { return $this->type::tryFrom($v); } catch (\\Error) { return null; } } }"),
('positive_static', "namespace App;\ninterface U {}\nclass Obj { public function has(): bool { return true; } }\n\nclass R { /** @var class-string<U> */ protected string $type = U::class;\n  public function f($v): mixed { if (method_exists($this->type, 'tryFrom')) { return $this->type::tryFrom($v); } return null; } }"),
('user_map', 'namespace App;\nclass Foo {}\n\n/**\n * @template V\n * @template U\n * @param list<V> $items\n * @param callable(V): U $map\n * @return list<U>\n */\nfunction map_all(array $items, callable $map): array { return array_map($map, $items); }\n/**\n * @template K of array-key\n * @template V\n * @param array<K, V> $items\n * @return list<K>\n */\nfunction keys_of(array $items): array { return array_keys($items); }\n/** @param array<string, int> $m */\nfunction f(array $m): void {\n    foreach (map_all([1, 2], fn (int $i): Foo => new Foo) as ‸{code: Foo $x}$x) {}\n    foreach (keys_of($m) as ‸{code: string $k}$k) {}\n}'),
]

MARK = re.compile(r"‸\{([^}]*)\}")


def place_markers(src):
    """The file without its markers, and each marker's one-based line, column and expectation."""
    marks, lines = [], []
    for number, line in enumerate(src.strip("\n").split("\n"), 1):
        while (found := MARK.search(line)) is not None:
            marks.append((number + 1, len(line[: found.start()]) + 1, found.group(1)))
            line = line[: found.start()] + line[found.end() :]
        lines.append(line)
    return "<?php\n" + "\n".join(lines) + "\n", marks


def holds(expectation, answer):
    return all(
        (want[1:] not in answer) if want.startswith("!") else (want in answer)
        for want in expectation.split("&&")
    )


def section(answer, name, following):
    found = re.search(rf"{name}\n(.*?)\n{following}", answer, re.S)
    return " | ".join(
        line.strip()
        for line in (found.group(1) if found else "").split("\n")
        if line.strip() and not line.strip().startswith(("<?php", "namespace", "{", "}"))
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="target/release/phrost")
    parser.add_argument("--only", default=None)
    args = parser.parse_args()
    binary = os.path.abspath(args.binary)
    root = tempfile.mkdtemp(prefix="php-magic-")
    passed = total = 0
    try:
        for name, src in CASES:
            if args.only and args.only not in name:
                continue
            directory = os.path.join(root, name)
            os.makedirs(directory)
            text, marks = place_markers(src)
            with open(os.path.join(directory, "a.php"), "w") as out:
                out.write(text)
            print(f"== {name}")
            for line, column, expectation in marks:
                total += 1
                answer = subprocess.run(
                    [binary, "query", directory, "a.php", str(line), str(column)],
                    capture_output=True,
                    text=True,
                ).stdout.split("\ndiagnostics")[0]
                ok = holds(expectation, answer) if expectation else True
                passed += ok
                print(
                    f"  {'ok  ' if ok else 'FAIL'} {line}:{column} want[{expectation}]"
                    f" goto[{section(answer, 'goto', 'hover')}]"
                    f" hover[{section(answer, 'hover', 'signature')[:160]}]"
                )
            checked = subprocess.run(
                [binary, "check", directory], capture_output=True, text=True
            ).stdout
            for finding in checked.split("\n"):
                if finding.startswith("a.php") and "unused-" not in finding:
                    print(f"  diag {finding}")
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print(f"\n{passed}/{total} markers pass")


if __name__ == "__main__":
    main()
