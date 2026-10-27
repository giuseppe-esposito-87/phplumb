<?php
namespace Drupal\mymodule;

use Drupal\Core\DependencyInjection\ContainerBuilder;
use Drupal\Core\DependencyInjection\ServiceProviderInterface;

class MymoduleServiceProvider implements ServiceProviderInterface
{
    public function register(ContainerBuilder $container)
    {
        $container->register('mymodule.dynamic', Greeter::class);
    }
}
