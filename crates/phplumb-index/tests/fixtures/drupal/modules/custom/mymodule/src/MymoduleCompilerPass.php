<?php
namespace Drupal\mymodule;

use Symfony\Component\DependencyInjection\Compiler\CompilerPassInterface;

class MymoduleCompilerPass implements CompilerPassInterface
{
    public function process($container)
    {
        $container->setAlias(GreeterFactory::class, 'mymodule.greeter');
    }
}
