package org.zeravrix.parsers.site.madara.ar

import org.zeravrix.parsers.MangaLoaderContext
import org.zeravrix.parsers.MangaSourceParser
import org.zeravrix.parsers.model.MangaParserSource
import org.zeravrix.parsers.site.madara.MadaraParser

@MangaSourceParser("AZORAMOON", "AzoraMoon", "ar")
internal class AzoraMoon(context: MangaLoaderContext) :
	MadaraParser(context, MangaParserSource.AZORAMOON, "azoramoon.com", pageSize = 10) {
	override val tagPrefix = "series-genre/"
	override val listUrl = "series/"
}
