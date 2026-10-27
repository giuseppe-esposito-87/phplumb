<?php
namespace Drupal\mymodule\Quiet;

class SiteServices
{
    public function run()
    {
        \Drupal::service('site.only');
    }
}
