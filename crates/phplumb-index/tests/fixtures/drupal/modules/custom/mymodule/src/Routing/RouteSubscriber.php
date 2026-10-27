<?php
namespace Drupal\mymodule\Routing;

use Symfony\Component\Routing\Route;
use Symfony\Component\Routing\RouteCollection;

class RouteSubscriber
{
    protected function alterRoutes(RouteCollection $collection)
    {
        $collection->add('mymodule.dynamic_route', new Route('/dynamic'));
        foreach (['thing'] as $type) {
            $collection->add('mymodule.dynamic.' . $type . '.view', new Route('/d'));
            $collection->add(sprintf('mymodule.sprintf.%s', $type), new Route('/s'));
        }
        foreach (['mymodule.page'] as $name) {
            $collection->add($name . '.json', new Route('/j'));
            $collection->add(sprintf('%1$s.xml', $name), new Route('/x'));
            $collection->add($name . '.' . $type, new Route('/t'));
        }
    }
}
