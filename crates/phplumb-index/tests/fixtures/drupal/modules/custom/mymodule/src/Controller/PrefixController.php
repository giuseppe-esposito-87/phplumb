<?php
namespace Drupal\mymodule\Controller;

use Symfony\Component\Routing\Attribute\Route;

#[Route(name: 'mymodule.prefix.')]
class PrefixController
{
    #[Route('/prefix/list', name: 'list')]
    public function list()
    {
    }
}
