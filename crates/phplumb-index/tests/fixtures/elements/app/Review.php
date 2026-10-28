<?php

namespace App;

/**
 * @param \Lib\Boxes<Item> $boxes
 * @param \Iterator<Item> $it
 * @param \Lib\Collection<Item> $c
 * @param list<list<Item>> $lists
 * @param list<Item> $items
 * @param \Lib\Collection<int, Item> $coll
 */
function review($boxes, \Iterator $it, $c, array $lists, array $items, $coll): void
{
    foreach ($boxes as $b) {
        $b->label();
    }
    $it->current()->label();
    $it->key()->label();
    one()->current()->label();
    one()->key()->label();
    foreach ($c as $ci) {
        $ci->label();
    }
    $c->key()->label();
    array_map(fn ($s) => $s[0]->label(), ...$lists);
    array_map(fn (...$vs) => $vs, $items);
    current($coll)->label();
    array_walk($coll, fn ($w) => $w->label());
}

/**
 * @return \Generator<Item>
 */
function one(): \Generator
{
    yield new Item();
}
