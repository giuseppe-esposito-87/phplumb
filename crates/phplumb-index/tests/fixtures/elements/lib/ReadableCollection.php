<?php

namespace Lib;

/**
 * @template TKey
 * @template-covariant T
 * @template-extends \IteratorAggregate<TKey, T>
 */
interface ReadableCollection extends \IteratorAggregate
{
    /**
     * @return T|false
     */
    public function first();

    /**
     * @return array<TKey, T>
     */
    public function toArray();

    /**
     * @return ReadableCollection<TKey, T>
     */
    public function filter(\Closure $p);
}
