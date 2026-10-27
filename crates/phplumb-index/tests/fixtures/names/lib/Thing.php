<?php
namespace Lib;

class /*@target:thing*/Thing
{
    public static function make(): static
    {
        return new static();
    }
}
