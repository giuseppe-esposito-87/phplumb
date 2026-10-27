<?php
namespace Drupal\mymodule;

use Drupal\Core\Field\BaseFieldDefinition;
use Drupal\Core\Url;
use Symfony\Component\DependencyInjection\ContainerInterface;

class Broken
{
    public function run(ContainerInterface $container, $variable): array
    {
        \Drupal::service('nowhere.service');
        $container->get('nowhere.container_service');
        Url::fromRoute('nowhere.route');
        BaseFieldDefinition::create('nowhere_field')
            ->setDisplayOptions('form', ['type' => 'nowhere_widget']);
        $form['a'] = ['#type' => 'nowhere_element'];

        \Drupal::service('mymodule.dynamic');
        \Drupal::service('test.only');
        $container->get('kernel');
        Url::fromRoute('mymodule.dynamic_route');
        Url::fromRoute('entity.node.canonical');
        Url::fromRoute('<front>');
        BaseFieldDefinition::create('string')->addConstraint('NotBlank');
        \Drupal::service('plugin.manager.block')->createInstance('menu_block:main');
        if ($container->has('guarded.service')) {
            $container->get('guarded.service');
        }
        \Drupal::service($variable);
        $variable->get('nowhere.key');
        $form['b'] = ['#type' => 'textfield'];
        return $form;
    }
}
