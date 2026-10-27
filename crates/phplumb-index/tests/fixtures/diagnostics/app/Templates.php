<?php
namespace App;

trait Helper
{
    public function help(): void
    {
        $this->host();
        self::alsoHost();
    }
}

class UsesHelper
{
    use Helper;
}

abstract class Template
{
    public function run(): void
    {
        $this->step();
        $this->/*@broken*/nowhere();
    }
}

class Concrete extends Template
{
    public function step(): void
    {
    }
}
