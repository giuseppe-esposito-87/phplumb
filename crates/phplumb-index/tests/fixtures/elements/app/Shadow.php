<?php

namespace App\Shadow;

function array_map(callable $f, array $a): array
{
    return [];
}

/**
 * @param list<\App\Item> $items
 */
function shadowed(array $items): void
{
    array_map(fn ($item) => $item->label(), $items);
}
