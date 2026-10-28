<?php

namespace App;

function concrete(Items $items): void
{
    foreach ($items as $concrete) {
        $concrete->label();
    }
}

/**
 * @param \Iterator<Item> $it
 */
function single(\Iterator $it): void
{
    foreach ($it as $one) {
        $one->label();
    }
    foreach (gen() as $g) {
        $g->label();
    }
}

/**
 * @return \Generator<int, Item>
 */
function gen(): \Generator
{
    yield new Item();
}

/**
 * @param list<Item>|null $maybe
 */
function nullable(?array $maybe): void
{
    foreach ($maybe as $m) {
        $m->label();
    }
    $maybe[0]->label();
}

/**
 * @param \Lib\LoopA<Item> $loop
 */
function cyclic($loop): void
{
    foreach ($loop as $l) {
        $l->label();
    }
}

/**
 * @param \Lib\Pair<Item> $pair
 */
function pair($pair): void
{
    $pair->first()->label();
    $pair->second()->label();
}
