<?php
namespace Lib;

interface HasLimit
{
    const /*@target:iconst*/LIMIT = 10;
}

trait Makes
{
    public static function /*@target:tmake*/fromTrait(): static
    {
        return new static();
    }
}

abstract class /*@target:base*/Base implements HasLimit
{
    public function /*@target:parentctor*/__construct()
    {
    }
}
