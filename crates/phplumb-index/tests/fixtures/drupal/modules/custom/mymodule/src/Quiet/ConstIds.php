<?php
namespace Drupal\mymodule\Quiet;

use Drupal\gadget\FooManager;

class ConstIds
{
    public function run(FooManager $foo)
    {
        $foo->createInstance('foo_const');
    }
}
