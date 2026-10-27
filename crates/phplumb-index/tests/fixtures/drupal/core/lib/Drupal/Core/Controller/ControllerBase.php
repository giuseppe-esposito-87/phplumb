<?php
namespace Drupal\Core\Controller;

abstract class ControllerBase
{
    protected function redirect($route_name, array $route_parameters = [], array $options = [], $status = 302)
    {
    }
}
