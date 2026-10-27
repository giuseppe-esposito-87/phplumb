<?php
namespace Drupal\mymodule;

class Greeter
{
    public function greet(): string
    {
        return 'hi';
    }
}

class GreeterDecorator
{
    public function greet(): string
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
