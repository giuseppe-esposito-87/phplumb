<?php

namespace App;

/**
 * @param list<Item> $items
 * @param list<string> $names
 */
function arrays(array $items, array $loose, array $names): void
{
    array_map(static fn ($item) => $item->label(), $items);
    array_map(fn ($n, $i) => $i->label(), $names, $items);
    array_filter($items, fn ($f) => $f->label());
    array_filter($items, fn ($b, $k) => $b->label(), ARRAY_FILTER_USE_BOTH);
    array_filter($items, fn ($key) => $key->label(), ARRAY_FILTER_USE_KEY);
    usort($items, fn ($a, $b) => $a->label() <=> $b->label());
    array_reduce($items, fn ($carry, $r) => $r->label(), '');
    array_reduce($items, fn ($carry, $r) => $carry->label(), '');
    array_map(fn (Other $o) => $o->label(), $items);
    array_map(fn ($x) => $x->label(), $loose);
    array_map(array: $items, callback: fn ($named) => $named->label());
    foreach (array_values(array_filter($items)) as $v) {
        $v->label();
    }
    reset($items)->label();
    array_map(fn ($y): Item => $y, $loose)[0]->label();
}
