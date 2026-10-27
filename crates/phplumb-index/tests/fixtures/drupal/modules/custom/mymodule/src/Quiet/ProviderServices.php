<?php
namespace Drupal\mymodule\Quiet;

class ProviderServices
{
    public function run()
    {
        \Drupal::service('mymodule.var');
        \Drupal::service('mymodule.bin.render');
        \Drupal::service('mymodule.auto');
    }
}
