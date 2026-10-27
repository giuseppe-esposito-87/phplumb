<?php
namespace App;

use Lib\Bar;
use Lib\Foo;
use Lib\UsesFluent;

class Outer extends Foo
{
    public function own(): void
    {
    }

    public function anonymous(): void
    {
        $object = new class extends Bar {
            public function /*@target:anon_own*/own(): void
            {
            }

            public function run(): void
            {
                $this->/*@cursor:bar_m*/m();
                $this->/*@cursor:anon_own*/own();
                self::/*@cursor:anon_own*/own();
                parent::/*@cursor:none*/onlyFoo();
            }
        };
    }
}

function closures(Foo $x, $e, $u): void
{
    $cb = function () {
        $x = new Bar();
    };
    $x->/*@cursor:foo_m*/m();

    $y = new Foo();
    $arrow = fn() => $y->/*@cursor:foo_m*/m();
    $used = function () use ($y) {
        $y->/*@cursor:foo_m*/m();
    };
    $unused = function () {
        $y->/*@cursor:none*/m();
    };

    if ($e instanceof Foo) {
        $shadow = fn($e) => $e->/*@cursor:none*/m();
        $typed = function (Bar $e) {
            $e->/*@cursor:bar_m*/m();
        };
        $e->/*@cursor:foo_m*/m();
        $e = new Bar();
        $e->/*@cursor:bar_m*/m();
    }

    if ($u) {
        $u = new Bar();
    } else {
        $u->/*@cursor:none*/m();
    }
}

function flow($c, array $pairs, array $arr): void
{
    $x = new Foo();
    $x->/*@cursor:foo_m*/m();
    $x = new Bar();
    $x->/*@cursor:bar_m*/m();

    $both = new Foo();
    if ($c) {
        $both = new Bar();
    }
    $both->/*@cursor:fm_union*/m();

    $late = NULL;
    if ($c) {
        $late = new Foo();
    }
    $late->/*@cursor:foo_m*/m();

    $maybe = unknown_call();
    if ($c) {
        $maybe = new Foo();
    }
    $maybe->/*@cursor:none*/m();

    $d = new Foo();
    [$d] = $pairs;
    $d->/*@cursor:none*/m();

    $k = new Foo();
    foreach ($arr as $k => $v) {
    }
    $k->/*@cursor:none*/m();

    $g = new Foo();
    global $g;
    $g->/*@cursor:none*/m();

    (new UsesFluent())->set()->/*@cursor:uses_other*/other();
    Foo::create()->/*@cursor:foo_m*/m();
}

function defines_leak(): void
{
    $leak = new Foo();
}

$leak->/*@cursor:none*/m();
