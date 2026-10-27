<?php
namespace Drupal\Core\Form;

interface FormStateInterface
{
    public function setRedirect($route_name, array $route_parameters = [], array $options = []);
}
