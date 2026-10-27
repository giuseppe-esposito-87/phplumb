<?php
namespace Drupal\Core\Form;

abstract class FormBase
{
    protected function redirect($route_name, array $route_parameters = [], array $options = [], $status = 302)
    {
    }
}
