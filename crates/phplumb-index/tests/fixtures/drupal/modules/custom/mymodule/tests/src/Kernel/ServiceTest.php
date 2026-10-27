<?php
namespace Drupal\Tests\mymodule\Kernel;

class ServiceTest
{
    public function testMissing(): void
    {
        \Drupal::service('nowhere.in_tests');
    }
}
