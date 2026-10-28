<?php

namespace Lib;

/**
 * @template T
 * @extends LoopB<T>
 */
interface LoopA extends LoopB
{
}

/**
 * @template T
 * @extends LoopA<T>
 */
interface LoopB extends LoopA
{
}
