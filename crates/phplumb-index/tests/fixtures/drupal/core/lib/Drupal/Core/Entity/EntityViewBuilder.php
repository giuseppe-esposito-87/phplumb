<?php
namespace Drupal\Core\Entity;

use Drupal\Core\Extension\ModuleHandlerInterface;

class EntityViewBuilder
{
    protected $entityTypeId;

    public function __construct(protected ModuleHandlerInterface $moduleHandler)
    {
    }

    public function buildMultiple(array $build_list)
    {
        $view_hook = "{$this->entityTypeId}_view";
        $this->moduleHandler->/*@target:thing_view*/invokeAll($view_hook, [$build_list]);
        $this->moduleHandler->invokeAll('entity_view', [$build_list]);
        $this->moduleHandler->/*@target:thing_view_alter*/alter([$view_hook, 'entity_view'], $build_list);
    }
}
