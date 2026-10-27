<?php
namespace Drupal\mymodule;

class Greeter
{
    public function /*@target:greet*/greet(): string
    {
        return 'hi';
    }
}

class GreeterDecorator
{
    public function /*@target:greet*/greet(): string
    {
        return 'hello';
    }
}

class SharedCustom
{
    public function share(): void
    {
    }
}
