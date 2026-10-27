<?php
namespace Drupal\mymodule\Quiet;

use Drupal\Core\Url;

class BuiltRoutes
{
    public function run()
    {
        Url::fromRoute('mymodule.page.json');
        Url::fromRoute('mymodule.page.xml');
    }
}
