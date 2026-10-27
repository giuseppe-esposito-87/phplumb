<?php
namespace Lib;

class Foo
{
    public function /*@target:foo_m*/ /*@target:fm_union*/m(): void
    {
    }

    public function onlyFoo(): void
    {
    }

    public static function create(): static
    {
        return new static();
    }
}

class Bar
{
    public function /*@target:bar_m*/ /*@target:fm_union*/m(): void
    {
    }
}

class Base
{
    public static function create(): static
    {
        return new static();
    }

    public function m(): void
    {
    }
}

class Sub extends Base
{
    public function /*@target:sub_m*/m(): void
    {
    }

    public function make(): void
    {
        $x = parent::create();
        $x->/*@cursor:sub_m*/m();
    }
}

trait Fluent
{
    public function set(): self
    {
        return $this;
    }

    public function other(): void
    {
    }
}

class UsesFluent
{
    use Fluent;

    public function /*@target:uses_other*/other(): void
    {
    }
}
