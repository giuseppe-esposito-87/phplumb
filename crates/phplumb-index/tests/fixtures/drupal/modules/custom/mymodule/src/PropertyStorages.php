<?php
namespace Drupal\mymodule;

use Drupal\Core\Entity\EntityStorageInterface;
use Drupal\Core\Entity\EntityTypeManagerInterface;

class PropertyStorages
{
    protected EntityStorageInterface $thingStorage;

    /**
     * @var \Drupal\Core\Entity\EntityStorageInterface
     */
    protected $gadgetStorage;

    protected EntityStorageInterface $plainStorage;

    protected $helper;

    public function __construct(EntityTypeManagerInterface $etm)
    {
        $this->thingStorage = $etm->getStorage('thing');
        $this->helper = new Greeter();
    }

    public static function create(EntityTypeManagerInterface $etm): static
    {
        $instance = new static($etm);
        $instance->gadgetStorage = $etm->getStorage('gadget');
        return $instance;
    }

    public function run(): void
    {
        $this->thingStorage->/*@cursor:thingCount*/thingCount();
        $this->thingStorage->load(1)->/*@cursor:thingName*/thingName();
        $this->gadgetStorage->load(1)->/*@cursor:gadgetName*/gadgetName();
        $this->helper->/*@cursor:greet_own*/greet();
        $this->plainStorage->load(1)->/*@cursor:id*/id();
    }
}

class ChildStorages extends PropertyStorages
{
    public function more(): void
    {
        $this->thingStorage->load(1)->/*@cursor:thingName*/thingName();
    }
}
