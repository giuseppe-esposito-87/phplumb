<?php
namespace Drupal\mymodule;

use Drupal\Core\Entity\EntityStorageInterface;
use Drupal\Core\Entity\EntityTypeManagerInterface;

class Entities
{
    public function run(EntityTypeManagerInterface $etm, EntityStorageInterface $plain, bool $flag): void
    {
        $etm->getStorage('thing')->/*@cursor:thingCount*/thingCount();
        $etm->getStorage('thing')->load(1)->/*@cursor:thingName*/thingName();
        $storage = $etm->getStorage('thing');
        if ($flag) {
            $storage->create([]);
        }
        $storage->load(2)->/*@cursor:thingName*/thingName();
        foreach ($storage->loadMultiple() as $thing) {
            $thing->/*@cursor:thingName*/thingName();
        }
        \Drupal::entityTypeManager()->getStorage('thing')->/*@cursor:thingCount*/thingCount();
        $etm->getStorage('gadget')->/*@cursor:sqlTable*/sqlTable();
        $etm->getStorage('gadget')->load(1)->/*@cursor:gadgetName*/gadgetName();
        $etm->getStorage('setting')->/*@cursor:configPrefix*/configPrefix();
        $plain->load(1)->/*@cursor:id*/id();
        $etm->getStorage('dual');
    }
}
