<?php
namespace Lib;

const LIMIT = 10;

function helper(): void
{
}

interface EntityInterface
{
    public function id();
}

interface FieldableInterface extends EntityInterface
{
}

abstract class EntityBase implements EntityInterface
{
    public function id()
    {
    }
}

class Content extends EntityBase implements FieldableInterface
{
    const TYPE = 'content';

    public function get($name)
    {
    }

    public static function create(): static
    {
        return new static();
    }
}

class Magic
{
    public function __call($name, $arguments)
    {
    }
}

class StaticMagic
{
    public static function __callStatic($name, $arguments)
    {
    }
}

/**
 * @method int virtual()
 */
class Documented
{
}

/**
 * @mixin Content
 */
class Mixed
{
}

class Failure extends \Exception
{
}

class NoConstructor
{
}

enum Suit: string
{
    case Hearts = 'h';
}

enum Plain
{
    case One;
}
