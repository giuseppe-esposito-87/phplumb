<?php
namespace Drupal\mymodule;

use Drupal\Core\DependencyInjection\ContainerBuilder;
use Drupal\Core\DependencyInjection\ServiceProviderBase;

class MymoduleAlterProvider extends ServiceProviderBase
{
    public function alter(ContainerBuilder $container)
    {
        $id = 'mymodule.var';
        $container->register($id, Greeter::class);
        foreach (['render'] as $bin) {
            $container->register("mymodule.bin.$bin", Greeter::class);
        }
    }
}
