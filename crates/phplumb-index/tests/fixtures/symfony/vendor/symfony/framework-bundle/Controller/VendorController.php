<?php

namespace Symfony\Bundle\FrameworkBundle\Controller;

use Symfony\Component\Routing\Attribute\Route;

class VendorController
{
    #[Route('/vendor', name: 'vendor_page')]
    public function page(): void
    {
    }
}
