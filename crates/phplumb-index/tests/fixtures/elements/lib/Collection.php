<?php

namespace Lib;

/**
 * @template TKey
 * @template T
 * @template-extends ReadableCollection<TKey, T>
 * @template-extends \ArrayAccess<TKey, T>
 */
interface Collection extends ReadableCollection, \ArrayAccess
{
}
