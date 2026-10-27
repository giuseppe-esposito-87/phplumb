<?php
namespace Drupal\mymodule;

class ServiceRegistrar
{
    public function setUp($builder)
    {
        $builder->autowire('mymodule.auto', Greeter::class);
    }
}
