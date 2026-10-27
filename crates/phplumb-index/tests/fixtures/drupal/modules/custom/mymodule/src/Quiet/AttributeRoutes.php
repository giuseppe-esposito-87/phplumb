<?php
namespace Drupal\mymodule\Quiet;

use Drupal\Core\Url;

class AttributeRoutes
{
    public function run()
    {
        Url::fromRoute('mymodule.prefix.list');
        Url::fromRoute('Drupal\mymodule\Controller\PrefixController::list');
        Url::fromRoute('mymodule.invoke');
        Url::fromRoute('Drupal\mymodule\Controller\InvokeController');
        Url::fromRoute('Drupal\mymodule\Controller\InvokeController::__invoke');
    }
}
