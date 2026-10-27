<?php
namespace Drupal\mymodule\Routing;

use Symfony\Component\Routing\Route;
use Symfony\Component\Routing\RouteCollection;

class RouteSubscriber
{
    protected function alterRoutes(RouteCollection $collection)
    {
        $collection->add('mymodule.dynamic_route', new Route('/dynamic'));
    }
}
