<?php
namespace Lib;

const /*@cursor:limit*/LIMIT = 10;

interface Saveable
{
    public function save(): void;
}

class /*@cursor:entity*/Entity implements Saveable
{
    public $prop;

    public function /*@cursor:save*/save(): void
    {
    }

    public static function /*@cursor:make*/make(): static
    {
        return new static();
    }

    public function touch(): void
    {
        $this->/*@target:prop*/prop = 1;
        echo $this->/*@target:prop*/ /*@cursor:prop*/prop;
    }
}

class Child extends /*@target:entity*/Entity
{
    public function store(): void
    {
        parent::/*@target:save*/save();
        $this->/*@target:save*/save();
    }
}

class Unrelated
{
    public function save(): void
    {
    }
}

function /*@cursor:helper*/helper(): void
{
}
