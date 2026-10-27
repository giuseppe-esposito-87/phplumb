<?php
namespace Drupal\mymodule;

class Greeter
{
    public function /*@target:greet*/ /*@target:greet_own*/greet(): string
    {
        return 'hi';
    }

    public function setManager($manager)
    {
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
