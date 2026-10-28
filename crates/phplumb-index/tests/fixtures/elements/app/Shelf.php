<?php

namespace App;

use Lib\Collection;

class Shelf
{
    /** @var Collection<int, Item> */
    private Collection $items;

    /** @var array<string, Item> */
    private array $byName;

    private array $loose;

    private Collection $plain;

    public function loops(string $k): void
    {
        foreach ($this->items as $item) {
            $item->label();
        }
        $this->byName['a']->label();
        ($this->byName[$k] ?? null)?->label();
        $this->items[0]->label();
        foreach ($this->loose as $x) {
            $x->label();
        }
        $this->loose[0]->label();
        foreach ($this->plain as $p) {
            $p->label();
        }
    }

    public function members(): void
    {
        $this->items->first()->label();
        foreach ($this->items->toArray() as $t) {
            $t->label();
        }
        $this->items->filter(fn () => true)->first()->label();
        $this->plain->first()->label();
    }
}
